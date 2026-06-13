package com.forge.app.domain.adapt

import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.mood.Mood
import kotlin.math.roundToInt

/**
 * System 5 of the adaptation engine: adaptive deload. Replaces the fixed 24-session
 * counter (`OverviewUiState.DELOAD_THRESHOLD`) with a transparent additive **fatigue
 * score** over the last [AdaptThresholds.deloadWindowDays] days. Each driver contributes
 * a named term; the suggestion's reason lists exactly the drivers that fired — never a
 * black-box number alone.
 *
 * Drivers (weights are rule constants, thresholds live in [AdaptThresholds]):
 *  - effort inflation (+2): HARD/BRUTAL share of rated bouts high while volume is flat/down
 *  - intra-session rep drop-off (+1): reps falling off steeply from first to last set
 *  - e1RM regression (+2): ≥ N lifts below the prior month's strength
 *  - low mood (+2): DRAINED/OFF in ≥ N of the last 5 mood entries
 *  - cardio rest reasons (+2 sick / +1 sore): the body is already asking for recovery
 *  - overdue (+1): no deload week in the last N weeks of training history
 *  - plateaus (+1): ≥ N lifts currently stalled (System 1's stall detection)
 *
 * Gates: ≥ [AdaptThresholds.deloadMinSessions] finished sessions AND at least one full
 * window of history; a deload inside the last [AdaptThresholds.deloadRecentDeloadSuppressDays]
 * days mutes the advisor entirely (recovery is already happening). Pure + deterministic.
 */
object DeloadAdvisor {

    private const val POINTS_EFFORT_INFLATION = 2
    private const val POINTS_REP_DROPOFF = 1
    private const val POINTS_E1RM_REGRESSION = 2
    private const val POINTS_LOW_MOOD = 2
    private const val POINTS_SICK = 2
    private const val POINTS_SORE = 1
    private const val POINTS_OVERDUE = 1
    private const val POINTS_PLATEAUS = 1

    private const val DAY_MS = 24L * 60 * 60 * 1000

    /** The driver breakdown behind a (potential) deload call — also feeds the sub-threshold "recovery signals building" insight. */
    data class FatigueAssessment(val score: Int, val drivers: List<String>)

    fun evaluate(s: AdaptationSnapshot, t: AdaptThresholds = AdaptThresholds()): Recommendation.DeloadSuggestion? {
        val f = fatigue(s, t) ?: return null
        if (f.score < t.deloadScoreThreshold) return null
        return Recommendation.DeloadSuggestion(
            score = f.score,
            drivers = f.drivers,
            reason = "Accumulated fatigue: " + f.drivers.joinToString(" · "),
            confidence = if (f.score >= t.deloadHighScore) Confidence.HIGH else Confidence.MEDIUM
        )
    }

    /** Score + named drivers, or null when the data gates fail / a deload just happened. */
    fun fatigue(s: AdaptationSnapshot, t: AdaptThresholds = AdaptThresholds()): FatigueAssessment? {
        val sessions = s.sessions
        if (sessions.size < t.deloadMinSessions) return null
        val windowStart = s.nowMs - t.deloadWindowDays * DAY_MS
        // Need at least one full window of history or the "trend" is just the whole dataset.
        if (sessions.first().startedAt > windowStart) return null

        // Most recent deload, from a tagged session OR the persisted apply marker — the marker covers
        // the gap right after an apply, before any deload-week session has been logged (seam fix #18).
        val lastDeloadAt = listOfNotNull(
            sessions.lastOrNull { it.sessionType == "deload" || it.deloadMarkedHere }?.startedAt,
            s.prefs.lastDeloadAppliedMs
        ).maxOrNull()
        if (lastDeloadAt != null && lastDeloadAt >= s.nowMs - t.deloadRecentDeloadSuppressDays * DAY_MS) return null

        val drivers = mutableListOf<Pair<Int, String>>()
        val windowBouts = s.exerciseHistory.values.flatten()
            .filter { it.sessionStartedAt >= windowStart && !it.skipped }

        // ── Effort inflation: working harder for the same (or less) output ────────
        val rated = windowBouts.mapNotNull { it.effort }
        if (rated.size >= t.deloadMinRatedBouts) {
            val hardShare = rated.count { it == EffortRating.HARD || it == EffortRating.BRUTAL }.toDouble() / rated.size
            val windowVolume = sessions.filter { it.startedAt >= windowStart }.sumOf { it.totalVolumeLb ?: 0.0 }
            val priorVolume = sessions
                .filter { it.startedAt >= windowStart - t.deloadWindowDays * DAY_MS && it.startedAt < windowStart }
                .sumOf { it.totalVolumeLb ?: 0.0 }
            if (hardShare >= t.deloadEffortShare && priorVolume > 0 && windowVolume <= priorVolume) {
                drivers += POINTS_EFFORT_INFLATION to
                    "${(hardShare * 100).roundToInt()}% of recent sets rated hard/brutal at flat volume"
            }
        }

        // ── Intra-session rep drop-off ─────────────────────────────────────────────
        val dropoffs = windowBouts.mapNotNull { bout ->
            val sets = bout.sets.filter { !it.isAssisted }
            if (sets.size < 3) return@mapNotNull null
            val first = sets.first().reps
            if (first <= 0) return@mapNotNull null
            (first - sets.last().reps).toDouble() / first
        }
        if (dropoffs.size >= t.deloadMinDropoffBouts) {
            val avg = dropoffs.average()
            if (avg >= t.deloadDropoffThreshold) {
                drivers += POINTS_REP_DROPOFF to
                    "reps dropping ~${(avg * 100).roundToInt()}% within sessions"
            }
        }

        // ── e1RM regression on multiple lifts ──────────────────────────────────────
        val priorStart = windowStart - 28 * DAY_MS
        val regressing = s.exerciseHistory.values.count { bouts ->
            val inWindow = bouts.filter { it.sessionStartedAt >= windowStart && !it.skipped }
            val prior = bouts.filter { it.sessionStartedAt in priorStart until windowStart && !it.skipped }
            val windowBest = bestE1rm(inWindow) ?: return@count false
            val priorBest = bestE1rm(prior) ?: return@count false
            windowBest < priorBest * t.deloadRegressionFraction
        }
        if (regressing >= t.deloadRegressionLifts) {
            drivers += POINTS_E1RM_REGRESSION to "$regressing lifts below last month's strength"
        }

        // ── Mood trend (leading recovery indicator) ────────────────────────────────
        val recentMoods = s.moods
            .filter { it.recordedAt >= s.nowMs - t.deloadWindowDays * DAY_MS }
            .sortedByDescending { it.recordedAt }
            .take(5)
        val lowMoods = recentMoods.count { Mood.fromCode(it.mood) in setOf(Mood.DRAINED, Mood.OFF) }
        if (lowMoods >= t.deloadLowMoodCount) {
            drivers += POINTS_LOW_MOOD to "mood low in $lowMoods of the last ${recentMoods.size} check-ins"
        }

        // ── Cardio rest reasons: the body already asked for recovery ──────────────
        val windowCardio = s.cardio.filter { it.date >= windowStart }
        when {
            windowCardio.any { it.restReason == "sick" } -> drivers += POINTS_SICK to "sick day logged recently"
            windowCardio.any { it.restReason == "sore" } -> drivers += POINTS_SORE to "soreness flagged on a rest day"
        }

        // ── Overdue: long stretch with no deload week ──────────────────────────────
        val overdueCutoff = s.nowMs - t.deloadNoDeloadWeeks * 7 * DAY_MS
        if ((lastDeloadAt ?: sessions.first().startedAt) <= overdueCutoff) {
            drivers += POINTS_OVERDUE to "no deload week in ${t.deloadNoDeloadWeeks}+ weeks"
        }

        // ── Plateaus (System 1's stall detection, reused) ──────────────────────────
        val plateauCount = ProgressionAdvisor.evaluate(s, t).size
        if (plateauCount >= t.deloadPlateauCount) {
            drivers += POINTS_PLATEAUS to "$plateauCount lifts plateaued"
        }

        return FatigueAssessment(score = drivers.sumOf { it.first }, drivers = drivers.map { it.second })
    }

    private fun bestE1rm(bouts: List<ExerciseBout>): Double? = bouts
        .flatMap { it.sets }
        .filter { it.weightLb != null && !it.isAssisted }
        .maxOfOrNull { E1rm.epley(it.weightLb!!, it.reps) }
}
