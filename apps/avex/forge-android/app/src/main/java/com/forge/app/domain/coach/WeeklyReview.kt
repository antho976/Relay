package com.forge.app.domain.coach

import com.forge.app.domain.adapt.AdaptThresholds
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.domain.adapt.ProgressionAdvisor
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.session.SessionType
import kotlin.math.roundToInt

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
    val focusLine: String,
    /** Resting heart rate from Health Connect, averaged over the same recent window the Overview
     *  recovery card uses (so the two agree); null when no samples fall in that window. */
    val restingHrBpm: Int? = null,
    /** True when Health Connect recovery signals (sleep / resting HR) were available for this read. */
    val usedHealthSignals: Boolean = false,
    /** Active (non-rest) cardio minutes logged last week — surfaced in the Brief's "Last week". */
    val cardioMinutesLastWeek: Int = 0
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

        // Off-app recovery signals (Health Connect, additive). Average resting HR over the SAME recent
        // window RestingHrTrend.spike uses for the Overview recovery card, so the Brief and the Overview
        // can't show a different bpm for the same data (averaging the full synced history diluted a
        // current spike). usedHealth flags that any HC signal was present so the Brief can say so.
        val hrWindowStart = s.nowMs - t.deloadWindowDays * DAY_MS
        val recentRestingHr = s.health.restingHr.filter { it.timeMs >= hrWindowStart }.map { it.bpm }
        val avgRestingHr = if (recentRestingHr.isEmpty()) null else recentRestingHr.average().roundToInt()
        val usedHealth = s.health.restingHr.isNotEmpty() || s.health.sleepNights.isNotEmpty()

        // Active cardio minutes logged in the same "last week" window (rest-day entries excluded).
        val cardioMinutesLastWeek = s.cardio
            .filter { it.date in lastWeekStart until weekStartMs && it.type != CardioType.REST.code }
            .sumOf { it.durationMin }

        val focus = when {
            hasDeloadShadow -> "Recovery is the work this week — go lighter, sleep more, and let the numbers reset."
            stalled > 0 -> "Chase the top of your rep ranges on the stalled lifts — finishing the range is what restarts progress."
            else -> mesocycleFocus(s, weekStartMs, t)
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
            focusLine = focus,
            restingHrBpm = avgRestingHr,
            usedHealthSignals = usedHealth,
            cardioMinutesLastWeek = cardioMinutesLastWeek
        )
    }

    /** Generic "the boring weeks build the most" line — used when there isn't a real block to read. */
    private const val GENERIC_FOCUS =
        "Keep doing what you're doing — fill the rep range, then add weight. Boring weeks build the most."

    /**
     * Mesocycle phase cue (auto-coach Phase 1 smarts): when nothing's stalled and no deload is due,
     * the most useful thing a coach can say tracks WHERE in the training block you are. The block is
     * anchored on the last deload (a tagged session, the in-place mark, or the persisted apply
     * marker), falling back to the first session; the week-in-block then maps to accumulate → build →
     * peak → "ease off soon". Below the deload data gate, or in week 0 of a fresh block, there isn't
     * enough of a block to name a phase, so it stays on the generic line.
     */
    private fun mesocycleFocus(s: AdaptationSnapshot, weekStartMs: Long, t: AdaptThresholds): String {
        if (s.sessions.size < t.deloadMinSessions) return GENERIC_FOCUS
        val anchor = listOfNotNull(
            s.sessions.lastOrNull { it.sessionType == SessionType.DELOAD.key || it.deloadMarkedHere }?.startedAt,
            s.prefs.lastDeloadAppliedMs,
            s.sessions.firstOrNull()?.startedAt
        ).maxOrNull() ?: return GENERIC_FOCUS
        val weeksIn = ((weekStartMs - anchor) / (7 * DAY_MS)).toInt()
        val meso = t.mesocycleWeeks
        return when {
            weeksIn < 1 -> GENERIC_FOCUS
            weeksIn == 1 -> "You're early in a fresh block — this is accumulation. Build reps and volume, keep the weights submaximal, and save the grinders for later."
            // No raw week count here: with no deload on record the anchor is the first session ever,
            // so weeksIn can span the whole training history — "it's been a while" stays honest where
            // "you're 150 weeks into this block" would not.
            weeksIn >= meso -> "It's been a good stretch with no down week — a lighter week soon will bank the gains; freshness is where they actually show up."
            weeksIn >= meso - 1 -> "Peak week of the block — go after rep PRs and your heaviest clean sets. This is where the work you've banked cashes in."
            else -> "Mid-block — the volume's in the bank, so intensify: finish the top of each range, then add load. This is the push phase."
        }
    }
}
