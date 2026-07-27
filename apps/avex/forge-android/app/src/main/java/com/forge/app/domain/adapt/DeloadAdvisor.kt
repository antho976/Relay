package com.forge.app.domain.adapt

import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.mood.Mood
import com.forge.app.domain.session.SessionType
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
 *  - cardio rest reasons (+2 sick / +1 sore): the body is already asking for recovery
 *  - sleep debt (+2): averaging below the nightly target over ≥ N nights (Health Connect)
 *  - elevated resting HR (+2): window resting HR ≥ N bpm above the prior month (Health Connect)
 *  - session mood (+1): most recent sessions rated drained/off (A1)
 *  - HRV suppression (+2): window RMSSD below the prior month's baseline (Health Connect, A1)
 *  - daily movement (+1): sustained high daily steps on top of training (Health Connect, A1)
 *  - overdue (+1): no deload week in the last N weeks of training history
 *  - plateaus (+1): ≥ N lifts currently stalled (System 1's stall detection)
 *
 * Every Health Connect driver is additive and gated: with no connected health data the
 * snapshot's [AdaptationSnapshot.health] is empty, so they contribute nothing. The mood driver
 * behaves the same way when the user never rates sessions.
 *
 * Bouts from test / technique / first-back sessions are excluded from every driver (A1) — they
 * are not ordinary training and would read as fatigue that isn't there.
 *
 * Gates: ≥ [AdaptThresholds.deloadMinSessions] finished sessions AND at least one full
 * window of history; a deload inside the last [AdaptThresholds.deloadRecentDeloadSuppressDays]
 * days mutes the advisor entirely (recovery is already happening). Pure + deterministic.
 */
object DeloadAdvisor {

    private const val POINTS_EFFORT_INFLATION = 2
    private const val POINTS_REP_DROPOFF = 1
    private const val POINTS_E1RM_REGRESSION = 2
    private const val POINTS_SICK = 2
    private const val POINTS_SORE = 1
    private const val POINTS_OVERDUE = 1
    private const val POINTS_PLATEAUS = 1
    /** Health Connect recovery signals (off-app), gated on the user having connected it. */
    private const val POINTS_SLEEP_DEBT = 2
    private const val POINTS_RESTING_HR = 2
    /** A1 drivers: the in-app mood log, plus the two W6 Health Connect series nothing read. */
    private const val POINTS_MOOD = 1
    private const val POINTS_HRV = 2
    private const val POINTS_DAILY_STEPS = 1

    private const val DAY_MS = 24L * 60 * 60 * 1000

    /** One named check behind the fatigue score: its live reading (always computed), and the
     *  points it adds when fired. Below a data gate the reading is the progress toward it. */
    data class FatigueCheck(
        val name: String,
        val reading: String,
        val points: Int,
        val fired: Boolean,
        /** True while the check is still short of its OWN data gate, so [reading] is progress toward
         *  it ("0 of 6 rated sets") rather than a live measurement — the UI collapses these. */
        val gated: Boolean = false,
        /** The full driver sentence when [fired] (feeds the deload reason); null when quiet. */
        val driver: String? = null
    )

    /** The driver breakdown behind a (potential) deload call — also feeds the sub-threshold "recovery signals building" insight. */
    data class FatigueAssessment(
        val score: Int,
        val drivers: List<String>,
        /** Every check with its live reading, fired or quiet — the read's full instrument panel. */
        val checks: List<FatigueCheck> = emptyList()
    )

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
        // A deload inside the suppress window mutes the advisor (recovery is already happening).
        val recentDeload = lastDeloadAt(s)
        if (recentDeload != null && recentDeload >= s.nowMs - t.deloadRecentDeloadSuppressDays * DAY_MS) return null

        val checks = instrument(s, t)
        return FatigueAssessment(
            score = checks.filter { it.fired }.sumOf { it.points },
            drivers = checks.mapNotNull { it.driver },
            checks = checks
        )
    }

    /**
     * The full instrument panel — every check with its live reading, computed regardless of the
     * deload gates. Surfaces what the coach tracks from session one; nothing "fires" toward a
     * score that isn't live yet, so a caller showing this pre-baseline reads the readings only.
     */
    fun checks(s: AdaptationSnapshot, t: AdaptThresholds = AdaptThresholds()): List<FatigueCheck> =
        instrument(s, t)

    /** Most recent deload, from a tagged session OR the persisted apply marker — the marker covers
     *  the gap right after an apply, before any deload-week session has been logged (seam fix #18). */
    private fun lastDeloadAt(s: AdaptationSnapshot): Long? = listOfNotNull(
        s.sessions.lastOrNull { it.sessionType == SessionType.DELOAD.key || it.deloadMarkedHere }?.startedAt,
        s.prefs.lastDeloadAppliedMs
    ).maxOrNull()

    /**
     * Builds every fatigue check (reading + fired + points + driver sentence when fired) from the
     * snapshot. Pure and ungated; [fatigue] applies the activation gates and sums the fired points.
     */
    private fun instrument(s: AdaptationSnapshot, t: AdaptThresholds): List<FatigueCheck> {
        val sessions = s.sessions
        val windowStart = s.nowMs - t.deloadWindowDays * DAY_MS
        val priorStart = windowStart - t.deloadPriorBaselineDays * DAY_MS
        val lastDeloadAt = lastDeloadAt(s)
        // Overdue measures from the last deload, or from the first session if none has happened.
        val sinceRef = lastDeloadAt ?: sessions.firstOrNull()?.startedAt ?: s.nowMs
        val checks = mutableListOf<FatigueCheck>()
        // A1: test/technique/first-back bouts are excluded everywhere fatigue is read — a light
        // technique day would look like effort inflation collapsing, a test day like a rep drop-off.
        val windowBouts = s.exerciseHistory.values.flatten()
            .filter { it.countsForProgression && it.sessionStartedAt >= windowStart && !it.skipped }

        // ── Effort inflation: working harder for the same (or less) output ────────
        val rated = windowBouts.mapNotNull { it.effort }
        val hardShare = if (rated.isEmpty()) 0.0
            else rated.count { it == EffortRating.HARD || it == EffortRating.BRUTAL }.toDouble() / rated.size
        val windowVolume = sessions.filter { it.startedAt >= windowStart }.sumOf { it.totalVolumeLb ?: 0.0 }
        val priorVolume = sessions
            .filter { it.startedAt >= windowStart - t.deloadWindowDays * DAY_MS && it.startedAt < windowStart }
            .sumOf { it.totalVolumeLb ?: 0.0 }
        val effortGated = rated.size < t.deloadMinRatedBouts
        val effortFired = !effortGated &&
            hardShare >= t.deloadEffortShare && priorVolume > 0 && windowVolume <= priorVolume
        checks += FatigueCheck(
            "Effort inflation",
            if (effortGated) "${rated.size} of ${t.deloadMinRatedBouts} rated sets"
            else "${(hardShare * 100).roundToInt()}% hard",
            POINTS_EFFORT_INFLATION, effortFired, gated = effortGated,
            driver = if (effortFired) "${(hardShare * 100).roundToInt()}% of recent sets rated hard/brutal at flat volume" else null
        )

        // ── Intra-session rep drop-off ─────────────────────────────────────────────
        val dropoffs = windowBouts.mapNotNull { bout ->
            val sets = bout.sets.filter { !it.isAssisted }
            if (sets.size < 3) return@mapNotNull null
            val first = sets.first().reps
            if (first <= 0) return@mapNotNull null
            (first - sets.last().reps).toDouble() / first
        }
        val dropAvg = if (dropoffs.isEmpty()) 0.0 else dropoffs.average()
        val dropGated = dropoffs.size < t.deloadMinDropoffBouts
        val dropFired = !dropGated && dropAvg >= t.deloadDropoffThreshold
        checks += FatigueCheck(
            "Rep drop-off",
            if (dropGated) "${dropoffs.size} of ${t.deloadMinDropoffBouts} bouts"
            else "~${(dropAvg * 100).roundToInt()}% fade",
            POINTS_REP_DROPOFF, dropFired, gated = dropGated,
            driver = if (dropFired) "reps dropping ~${(dropAvg * 100).roundToInt()}% within sessions" else null
        )

        // ── e1RM regression on multiple lifts ──────────────────────────────────────
        val regressing = s.exerciseHistory.values.count { bouts ->
            val training = bouts.filter { it.countsForProgression && !it.skipped }
            val inWindow = training.filter { it.sessionStartedAt >= windowStart }
            val prior = training.filter { it.sessionStartedAt in priorStart until windowStart }
            val windowBest = bestE1rm(inWindow) ?: return@count false
            val priorBest = bestE1rm(prior) ?: return@count false
            windowBest < priorBest * t.deloadRegressionFraction
        }
        val regressionFired = regressing >= t.deloadRegressionLifts
        checks += FatigueCheck(
            "Strength regression",
            "$regressing lift${if (regressing == 1) "" else "s"} down",
            POINTS_E1RM_REGRESSION, regressionFired,
            driver = if (regressionFired) "$regressing lifts below last month's strength" else null
        )

        // ── Cardio rest reasons: the body already asked for recovery ──────────────
        val windowCardio = s.cardio.filter { it.date >= windowStart }
        val sick = windowCardio.any { it.restReason == "sick" }
        val sore = windowCardio.any { it.restReason == "sore" }
        checks += FatigueCheck(
            "Rest-day flags",
            when { sick -> "sick day logged"; sore -> "soreness flagged"; else -> "none flagged" },
            if (sore && !sick) POINTS_SORE else POINTS_SICK, sick || sore,
            driver = when {
                sick -> "sick day logged recently"
                sore -> "soreness flagged on a rest day"
                else -> null
            }
        )

        // ── Sleep debt (Health Connect): chronically short nights blunt recovery ──
        // Additive + gated: only fires when the user has connected Health Connect AND there are
        // enough nights in the window to mean something — no HC data, no driver, no behavior change.
        val windowSleep = s.health.sleepNights.filter { it.endedAtMs >= windowStart }
        val sleepAvgMin = if (windowSleep.isEmpty()) 0.0 else windowSleep.map { it.durationMin }.average()
        val sleepGated = windowSleep.isNotEmpty() && windowSleep.size < t.deloadMinSleepNights
        val sleepFired = windowSleep.size >= t.deloadMinSleepNights && sleepAvgMin <= t.deloadSleepDebtMinutes
        checks += FatigueCheck(
            "Sleep debt",
            when {
                windowSleep.isEmpty() -> "no data"
                sleepGated -> "${windowSleep.size} of ${t.deloadMinSleepNights} nights"
                else -> "${String.format(java.util.Locale.US, "%.1f", sleepAvgMin / 60.0)}h avg"
            },
            POINTS_SLEEP_DEBT, sleepFired, gated = sleepGated,
            // Locale.US so the decimal point matches the app's English copy (fr devices use ',').
            driver = if (sleepFired) "averaging ${String.format(java.util.Locale.US, "%.1f", sleepAvgMin / 60.0)}h sleep over ${windowSleep.size} nights" else null
        )

        // ── Elevated resting HR (Health Connect) vs your own baseline ─────────────
        // The body's classic "not recovered" tell: window resting HR meaningfully above the prior
        // month's average. Compared against each user's OWN baseline, never an absolute number.
        val windowHr = s.health.restingHr.filter { it.timeMs >= windowStart }
        val priorHr = s.health.restingHr.filter { it.timeMs in priorStart until windowStart }
        val hrGated = windowHr.size >= t.deloadMinRestingHrSamples && priorHr.size >= t.deloadMinRestingHrSamples
        val hrDelta = if (hrGated) windowHr.map { it.bpm }.average() - priorHr.map { it.bpm }.average() else 0.0
        val hrFired = hrGated && priorHr.map { it.bpm }.average() > 0 && hrDelta >= t.deloadRestingHrDeltaBpm
        val hrBuilding = windowHr.isNotEmpty() && !hrGated
        checks += FatigueCheck(
            "Resting heart rate",
            when {
                windowHr.isEmpty() -> "no data"
                hrBuilding -> "${windowHr.size} of ${t.deloadMinRestingHrSamples} readings"
                else -> "${if (hrDelta >= 0) "+" else ""}${hrDelta.roundToInt()} bpm"
            },
            POINTS_RESTING_HR, hrFired, gated = hrBuilding,
            driver = if (hrFired) "resting HR up ${hrDelta.roundToInt()} bpm vs your baseline" else null
        )

        // ── Post-session mood (A1) ────────────────────────────────────────────────
        // Moods were captured since v1 and read by nobody. How training *feels* is the cheapest
        // fatigue signal there is, and the research on subjective monitoring says it outperforms
        // most gadgets. Gated on enough ratings in the window so one rough day never fires it.
        val windowMoods = s.moods.filter { it.recordedAt >= windowStart }
        val lowMoods = windowMoods.count { m ->
            val mood = Mood.fromCode(m.mood)
            mood == Mood.DRAINED || mood == Mood.OFF
        }
        val moodShare = if (windowMoods.isEmpty()) 0.0 else lowMoods.toDouble() / windowMoods.size
        val moodGated = windowMoods.isNotEmpty() && windowMoods.size < t.deloadMinMoods
        val moodFired = windowMoods.size >= t.deloadMinMoods && moodShare >= t.deloadLowMoodShare
        checks += FatigueCheck(
            "Session mood",
            when {
                windowMoods.isEmpty() -> "no data"
                moodGated -> "${windowMoods.size} of ${t.deloadMinMoods} ratings"
                else -> "$lowMoods of ${windowMoods.size} rough"
            },
            POINTS_MOOD, moodFired, gated = moodGated,
            driver = if (moodFired) "$lowMoods of your last ${windowMoods.size} sessions felt drained or off" else null
        )

        // ── HRV vs your own baseline (A1, Health Connect W6) ──────────────────────
        // Overnight RMSSD is noisy night-to-night and meaningful as a trend, so this compares the
        // window average against the prior month's — never an absolute number, never a single night.
        val windowHrv = s.health.hrv.filter { it.timeMs >= windowStart }
        val priorHrv = s.health.hrv.filter { it.timeMs in priorStart until windowStart }
        val hrvGated = windowHrv.isNotEmpty() &&
            (windowHrv.size < t.deloadMinHrvSamples || priorHrv.size < t.deloadMinHrvSamples)
        val hrvReady = windowHrv.size >= t.deloadMinHrvSamples && priorHrv.size >= t.deloadMinHrvSamples
        val priorHrvAvg = if (hrvReady) priorHrv.map { it.rmssdMs }.average() else 0.0
        val windowHrvAvg = if (hrvReady) windowHrv.map { it.rmssdMs }.average() else 0.0
        val hrvDropPct = if (hrvReady && priorHrvAvg > 0) (priorHrvAvg - windowHrvAvg) / priorHrvAvg else 0.0
        val hrvFired = hrvReady && priorHrvAvg > 0 && hrvDropPct >= t.deloadHrvDropFraction
        checks += FatigueCheck(
            "Heart-rate variability",
            when {
                windowHrv.isEmpty() -> "no data"
                hrvGated -> "${windowHrv.size} of ${t.deloadMinHrvSamples} readings"
                else -> "${if (hrvDropPct > 0) "−" else "+"}${(kotlin.math.abs(hrvDropPct) * 100).roundToInt()}%"
            },
            POINTS_HRV, hrvFired, gated = hrvGated,
            driver = if (hrvFired) "HRV down ${(hrvDropPct * 100).roundToInt()}% against your own baseline" else null
        )

        // ── Off-gym movement (A1, Health Connect W6) ──────────────────────────────
        // A very active life is training the coach can't see. Sustained high daily steps means the
        // recovery budget is already being spent, so the same lifting load costs more.
        val windowSteps = s.health.dailySteps.filter { it.dayStartMs >= windowStart }
        val stepsAvg = if (windowSteps.isEmpty()) 0.0 else windowSteps.map { it.steps }.average()
        val stepsGated = windowSteps.isNotEmpty() && windowSteps.size < t.deloadMinStepDays
        val stepsFired = windowSteps.size >= t.deloadMinStepDays && stepsAvg >= t.deloadHighDailySteps
        checks += FatigueCheck(
            "Daily movement",
            when {
                windowSteps.isEmpty() -> "no data"
                stepsGated -> "${windowSteps.size} of ${t.deloadMinStepDays} days"
                else -> "${stepsAvg.roundToInt()} steps/day"
            },
            POINTS_DAILY_STEPS, stepsFired, gated = stepsGated,
            driver = if (stepsFired) "averaging ${stepsAvg.roundToInt()} steps a day on top of training" else null
        )

        // ── Overdue: long stretch with no deload week ──────────────────────────────
        val overdueCutoff = s.nowMs - t.deloadNoDeloadWeeks * 7 * DAY_MS
        val overdueFired = sinceRef <= overdueCutoff
        val sinceWeeks = ((s.nowMs - sinceRef) / (7 * DAY_MS)).toInt()
        checks += FatigueCheck(
            "Time since deload",
            if (lastDeloadAt == null) "none logged" else "$sinceWeeks wk${if (sinceWeeks == 1) "" else "s"} ago",
            POINTS_OVERDUE, overdueFired,
            driver = if (overdueFired) "no deload week in ${t.deloadNoDeloadWeeks}+ weeks" else null
        )

        // ── Plateaus (System 1's stall detection, reused) ──────────────────────────
        val plateauCount = ProgressionAdvisor.evaluate(s, t).size
        val plateauFired = plateauCount >= t.deloadPlateauCount
        checks += FatigueCheck(
            "Plateaued lifts",
            "$plateauCount lift${if (plateauCount == 1) "" else "s"}",
            POINTS_PLATEAUS, plateauFired,
            driver = if (plateauFired) "$plateauCount lifts plateaued" else null
        )

        return checks
    }

    private fun bestE1rm(bouts: List<ExerciseBout>): Double? = bouts
        .flatMap { it.sets }
        .filter { it.weightLb != null && !it.isAssisted }
        .maxOfOrNull { E1rm.epley(it.weightLb!!, it.reps) }
}
