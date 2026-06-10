package com.forge.app.domain.adapt

import com.forge.app.domain.mood.Mood
import com.forge.app.program.DayPlan
import com.forge.app.program.Difficulty
import com.forge.app.program.ExercisePlan
import com.forge.app.program.MuscleGroup
import com.forge.app.program.SessionEstimate
import java.time.Instant
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * System 4 of the adaptation engine: pure observations over the snapshot. Insights have no
 * apply action — they render as inline rows on Stats. Absorbs the legacy `buildInsights`
 * rules (best time-of-day #40, most-improved #41, muscle dominance) — the old volume-drop
 * deload rule (#80) is intentionally NOT ported: System 5's DeloadAdvisor supersedes it
 * with a proper multi-signal score.
 *
 * Every rule is confidence-gated by [AdaptThresholds] — on sparse data a rule says nothing.
 * Pure + deterministic: time and zone come from the snapshot.
 */
object InsightEngine {

    private const val DAY_MS = 24L * 60 * 60 * 1000

    private val PUSH = setOf(MuscleGroup.CHEST, MuscleGroup.SHOULDERS, MuscleGroup.TRICEPS)
    private val PULL = setOf(MuscleGroup.BACK, MuscleGroup.REAR_DELTS, MuscleGroup.BICEPS)

    fun evaluate(s: AdaptationSnapshot, t: AdaptThresholds = AdaptThresholds()): List<Recommendation.Insight> {
        val slots = s.program.flatMap { it.slots }.associateBy { it.exerciseId }
        return listOfNotNull(
            bestTimeOfDay(s, t),
            mostImproved(s, slots, t),
            muscleDominance(s, slots, t),
            ratioInsight(s, slots, t, "pushpull", "Push/pull balance", PUSH, PULL, t.insightPushPullLow, t.insightPushPullHigh, "push", "pull"),
            ratioInsight(s, slots, t, "quadham", "Quad/ham balance", setOf(MuscleGroup.QUADS), setOf(MuscleGroup.HAMSTRINGS), t.insightQuadHamLow, t.insightQuadHamHigh, "quad", "hamstring"),
            mostSkipped(s, slots, t),
            repeatedSessionSwaps(s, slots, t),
            recoverySignalsBuilding(s, t),
            moodVolumeLink(s, t),
            estimateCalibration(s, t)
        )
    }

    // ── Ported: best time-of-day (#40) ─────────────────────────────────────────

    private fun bestTimeOfDay(s: AdaptationSnapshot, t: AdaptThresholds): Recommendation.Insight? {
        val bouts = s.exerciseHistory.values.flatten()
        val totalSets = bouts.sumOf { it.sets.size }
        if (totalSets < t.insightTimeOfDayMinSets) return null
        val byHour = bouts.groupBy { Instant.ofEpochMilli(it.sessionStartedAt).atZone(s.zoneId).hour }
            .mapValues { (_, b) -> b.sumOf { it.sets.size } }
        val best = byHour.maxByOrNull { it.value } ?: return null
        val label = when {
            best.key < 10 -> "morning"
            best.key < 13 -> "late morning"
            best.key < 17 -> "afternoon"
            else -> "evening"
        }
        return insight("timeofday", "Best time to train", "You log the most sets in the $label (${best.key}:00).", "⏰")
    }

    // ── Ported: most improved (#41) ────────────────────────────────────────────

    private fun mostImproved(
        s: AdaptationSnapshot,
        slots: Map<String, ProgramSlotSnap>,
        t: AdaptThresholds
    ): Recommendation.Insight? {
        val since = s.nowMs - 90 * DAY_MS
        val best = s.exerciseHistory.mapNotNull { (exerciseId, bouts) ->
            val name = slots[exerciseId]?.name ?: return@mapNotNull null
            val perSession = bouts
                .filter { it.sessionStartedAt >= since && !it.skipped }
                .mapNotNull { b -> b.sets.mapNotNull { it.weightLb }.maxOrNull() }
            if (perSession.size < t.insightImprovedMinSessions) return@mapNotNull null
            val mid = perSession.size / 2
            val first = perSession.take(mid).maxOrNull() ?: return@mapNotNull null
            val last = perSession.drop(mid).maxOrNull() ?: return@mapNotNull null
            if (first <= 0) return@mapNotNull null
            Triple(name, ((last - first) / first * 100).toInt(), exerciseId)
        }.maxByOrNull { it.second } ?: return null
        if (best.second <= t.insightImprovedMinPct) return null
        return insight("improved", "Most improved", "${best.first} is up ~${best.second}% in 3 months.", "📈")
    }

    // ── Ported: weekly muscle dominance ────────────────────────────────────────

    private fun muscleDominance(
        s: AdaptationSnapshot,
        slots: Map<String, ProgramSlotSnap>,
        t: AdaptThresholds
    ): Recommendation.Insight? {
        val weekStart = s.nowMs - 7 * DAY_MS
        val weekBouts = s.exerciseHistory.entries.flatMap { (id, bouts) ->
            val muscle = slots[id]?.muscle ?: return@flatMap emptyList()
            bouts.filter { it.sessionStartedAt >= weekStart }.map { muscle to it }
        }
        val totalSets = weekBouts.sumOf { it.second.sets.size }
        if (totalSets < t.insightBalanceMinWeekSets) return null
        val volumeByMuscle = weekBouts
            .groupBy({ it.first }, { it.second })
            .mapValues { (_, bouts) -> bouts.sumOf { b -> b.sets.sumOf { (it.weightLb ?: 0.0) * it.reps } } }
        val total = volumeByMuscle.values.sum()
        if (total <= 0) return null
        val dominant = volumeByMuscle.maxByOrNull { it.value } ?: return null
        if (dominant.value / total <= t.insightBalanceDominantShare) return null
        return insight(
            "dominance", "Muscle balance",
            "${dominant.key.displayName} is over ${(t.insightBalanceDominantShare * 100).roundToInt()}% of this week's volume — consider balancing.", "⚖️"
        )
    }

    // ── Structural ratios: push/pull, quad/ham ─────────────────────────────────

    private fun ratioInsight(
        s: AdaptationSnapshot,
        slots: Map<String, ProgramSlotSnap>,
        t: AdaptThresholds,
        key: String,
        title: String,
        sideA: Set<MuscleGroup>,
        sideB: Set<MuscleGroup>,
        low: Double,
        high: Double,
        labelA: String,
        labelB: String
    ): Recommendation.Insight? {
        val since = s.nowMs - t.insightRatioWindowDays * DAY_MS
        var a = 0
        var b = 0
        s.exerciseHistory.forEach { (id, bouts) ->
            val muscle = slots[id]?.muscle ?: return@forEach
            val sets = bouts.filter { it.sessionStartedAt >= since && !it.skipped }.sumOf { it.sets.size }
            when (muscle) {
                in sideA -> a += sets
                in sideB -> b += sets
                else -> {}
            }
        }
        if (a < t.insightRatioMinSetsPerSide || b < t.insightRatioMinSetsPerSide) return null
        val ratio = a.toDouble() / b
        if (ratio in low..high) return null
        val skew = if (ratio > high) labelA else labelB
        return insight(
            key, title,
            "Last ${t.insightRatioWindowDays} days: $a $labelA sets vs $b $labelB — leaning $skew-heavy.", "⚖️"
        )
    }

    // ── Adherence: chronically skipped / always session-swapped ────────────────

    private fun mostSkipped(
        s: AdaptationSnapshot,
        slots: Map<String, ProgramSlotSnap>,
        t: AdaptThresholds
    ): Recommendation.Insight? {
        val worst = slots.values.mapNotNull { slot ->
            val recent = s.exerciseHistory[slot.exerciseId]?.takeLast(t.insightAdherenceWindow) ?: return@mapNotNull null
            if (recent.size < t.insightAdherenceWindow) return@mapNotNull null
            val skips = recent.count { it.skipped }
            if (skips >= t.insightSkipCount) Triple(slot.name, skips, recent.size) else null
        }.maxByOrNull { it.second } ?: return null
        return insight(
            "skip.${worst.first}", "Often skipped",
            "${worst.first} was skipped ${worst.second} of the last ${worst.third} times — swap it for something you'll actually do, or drop it.", "🚪"
        )
    }

    private fun repeatedSessionSwaps(
        s: AdaptationSnapshot,
        slots: Map<String, ProgramSlotSnap>,
        t: AdaptThresholds
    ): Recommendation.Insight? {
        val worst = slots.values.mapNotNull { slot ->
            val recent = s.exerciseHistory[slot.exerciseId]?.takeLast(t.insightAdherenceWindow) ?: return@mapNotNull null
            val swapped = recent.mapNotNull { it.swappedName }
            if (swapped.size < t.insightSwapCount) return@mapNotNull null
            val favourite = swapped.groupingBy { it }.eachCount().maxByOrNull { it.value } ?: return@mapNotNull null
            Triple(slot.name, favourite.key, swapped.size)
        }.maxByOrNull { it.third } ?: return null
        return insight(
            "swap.${worst.first}", "Make the swap permanent?",
            "You've swapped ${worst.first} (usually to ${worst.second}) in ${worst.third} of your last ${t.insightAdherenceWindow} sessions — set it as a persistent swap, or dislike the original.", "🔁"
        )
    }

    // ── Under-recovery: System 5's score, just below the firing line ───────────

    private fun recoverySignalsBuilding(s: AdaptationSnapshot, t: AdaptThresholds): Recommendation.Insight? {
        val f = DeloadAdvisor.fatigue(s, t) ?: return null
        if (f.score >= t.deloadScoreThreshold || f.score < t.deloadScoreThreshold - 2) return null
        return insight(
            "recovery", "Recovery signals building",
            "Not deload territory yet, but: ${f.drivers.joinToString(" · ")}.", "🪫"
        )
    }

    // ── Mood × volume link ─────────────────────────────────────────────────────

    private fun moodVolumeLink(s: AdaptationSnapshot, t: AdaptThresholds): Recommendation.Insight? {
        val volumeBySession = s.sessions.associate { it.id to (it.totalVolumeLb ?: 0.0) }
        val pairs = s.moods.mapNotNull { m ->
            val vol = m.sessionId?.let(volumeBySession::get)?.takeIf { it > 0 } ?: return@mapNotNull null
            Mood.fromCode(m.mood)?.let { it to vol }
        }
        if (pairs.size < t.insightMoodPairs) return null
        val goodVols = pairs.filter { it.first == Mood.GOOD || it.first == Mood.STRONG }.map { it.second }
        val lowVols = pairs.filter { it.first == Mood.OFF || it.first == Mood.DRAINED }.map { it.second }
        if (goodVols.size < 3 || lowVols.size < 3) return null
        val gap = goodVols.average() / lowVols.average() - 1
        if (gap < t.insightMoodVolumeDiff) return null
        return insight(
            "moodvolume", "Mood moves your volume",
            "Sessions you rated good/strong average ${(gap * 100).roundToInt()}% more volume than off/drained ones — recovery is performance.", "💪"
        )
    }

    // ── Session-estimate calibration (zero new data, immediately useful) ──────

    private fun estimateCalibration(s: AdaptationSnapshot, t: AdaptThresholds): Recommendation.Insight? {
        val worst = s.program.mapNotNull { day ->
            val durations = s.sessions
                .filter { it.dayKey == day.dayKey && it.finishedAt != null }
                .mapNotNull { sess -> ((sess.finishedAt!! - sess.startedAt) / 60_000).toInt().takeIf { it in 10..240 } }
                .takeLast(t.insightEstimateMinSessions + 1)
            if (durations.size < t.insightEstimateMinSessions) return@mapNotNull null
            val actual = durations.sorted()[durations.size / 2]
            val estimate = SessionEstimate.estimateMinutes(day.toDayPlan())
            if (estimate <= 0) return@mapNotNull null
            Triple(day.name, actual, estimate)
        }.maxByOrNull { abs(it.second - it.third) } ?: return null
        if (abs(worst.second - worst.third) < t.insightEstimateDriftMinutes) return null
        return insight(
            "estimate.${worst.first}", "Estimate vs reality",
            "${worst.first} actually runs ~${worst.second} min (estimated ~${worst.third}).", "⏱️"
        )
    }

    /** Rebuild a minimal DayPlan from snapshot slots so SessionEstimate can price it. */
    private fun ProgramDaySnap.toDayPlan() = DayPlan(
        key = dayKey, defaultName = name, subtitle = "", word = "", accentHex = "",
        warmup = emptyList(),
        exercises = slots.map { slot ->
            ExercisePlan(
                id = slot.exerciseId, name = slot.name, sets = slot.targetSets,
                reps = slot.repsText, unit = slot.unit, muscle = slot.muscle,
                difficulty = Difficulty.BEGINNER, note = "", tags = slot.tags
            )
        }
    )

    private fun insight(key: String, title: String, body: String, icon: String) =
        Recommendation.Insight(key = key, title = title, body = body, icon = icon, confidence = Confidence.MEDIUM)
}
