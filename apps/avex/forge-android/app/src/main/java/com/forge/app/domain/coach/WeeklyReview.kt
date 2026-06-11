package com.forge.app.domain.coach

import com.forge.app.domain.adapt.AdaptThresholds
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.domain.adapt.ProgressionAdvisor

/** The "Last week" half of the Week Brief — numbers a coach would open with. */
data class WeeklyReviewData(
    val sessionsLastWeek: Int,
    val sessionsTarget: Int,
    val volumeLastWeekLb: Double,
    val volumePriorWeekLb: Double,
    val prsLastWeek: Int,
    val trackedLifts: Int,
    val stalledLifts: Int,
    /** DeloadAdvisor's fatigue score; null = its data gates aren't met yet. */
    val fatigueScore: Int?,
    /** "Fresh" | "Building" | "Deload soon" | "No read yet" — same bands as the Stats pulse. */
    val fatigueBand: String,
    /** The single most useful cue for the coming week. */
    val focusLine: String
) {
    /** Volume change vs the prior week in percent, null when there's no prior baseline. */
    val volumeDeltaPct: Int?
        get() = if (volumePriorWeekLb > 0)
            (((volumeLastWeekLb - volumePriorWeekLb) / volumePriorWeekLb) * 100).toInt()
        else null
}

/**
 * Pure assembler for the Week Brief's review section (auto-coach Phase 1). "Last week" is
 * the 7 days before [weekStartMs] (the Monday the Brief is published); the prior week is
 * the 7 days before that.
 */
object WeeklyReview {

    private const val DAY_MS = 24L * 60 * 60 * 1000

    fun assemble(
        s: AdaptationSnapshot,
        weekStartMs: Long,
        sessionsTarget: Int,
        hasDeloadShadow: Boolean,
        t: AdaptThresholds = AdaptThresholds()
    ): WeeklyReviewData {
        val lastWeekStart = weekStartMs - 7 * DAY_MS
        val priorWeekStart = weekStartMs - 14 * DAY_MS

        val lastWeek = s.sessions.filter { it.startedAt in lastWeekStart until weekStartMs }
        val priorWeek = s.sessions.filter { it.startedAt in priorWeekStart until lastWeekStart }

        // PRs are computed, not stored: a last-week bout whose top working weight beats every
        // earlier bout of that exercise (PrDetector's rule; a first-ever bout doesn't count).
        val prs = s.exerciseHistory.values.sumOf { bouts ->
            var best: Double? = null
            var count = 0
            bouts.sortedBy { it.sessionStartedAt }.forEach { b ->
                if (b.skipped) return@forEach
                val top = b.sets.filter { !it.isAssisted }.mapNotNull { it.weightLb }.maxOrNull()
                    ?: return@forEach
                val prior = best
                if (prior != null && top > prior && b.sessionStartedAt in lastWeekStart until weekStartMs) count++
                if (prior == null || top > prior) best = top
            }
            count
        }

        val stalled = ProgressionAdvisor.evaluate(s, t).map { it.id }.distinct().size
        val tracked = s.exerciseHistory.count { (_, bouts) ->
            bouts.count { b -> !b.skipped && b.sets.any { it.weightLb != null && !it.isAssisted } } > t.plateauMinBouts
        }

        val fatigue = DeloadAdvisor.fatigue(s, t)
        val band = when {
            fatigue == null -> "No read yet"
            fatigue.score >= t.deloadScoreThreshold -> "Deload soon"
            fatigue.score >= t.deloadScoreThreshold - 2 -> "Building"
            else -> "Fresh"
        }

        val focus = when {
            hasDeloadShadow -> "Recovery is the work this week — go lighter, sleep more, and let the numbers reset."
            stalled > 0 -> "Chase the top of your rep ranges on the stalled lifts — finishing the range is what restarts progress."
            else -> "Keep doing what you're doing — fill the rep range, then add weight. Boring weeks build the most."
        }

        return WeeklyReviewData(
            sessionsLastWeek = lastWeek.size,
            sessionsTarget = sessionsTarget,
            volumeLastWeekLb = lastWeek.sumOf { it.totalVolumeLb ?: 0.0 },
            volumePriorWeekLb = priorWeek.sumOf { it.totalVolumeLb ?: 0.0 },
            prsLastWeek = prs,
            trackedLifts = tracked,
            stalledLifts = stalled,
            fatigueScore = fatigue?.score,
            fatigueBand = band,
            focusLine = focus
        )
    }
}
