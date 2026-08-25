package com.forge.app.domain.coach

import com.forge.app.domain.adapt.AdaptThresholds
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.domain.adapt.ProgressionAdvisor
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.session.SessionType
import java.time.Instant
import java.time.ZoneId
import java.time.temporal.ChronoUnit
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
 * Pure assembler for the Week Brief's review section (auto-coach Phase 1). "Last week" is the
 * calendar week before [weekStartMs] (the Monday the Brief is published); the prior week is the
 * calendar week before that. Calendar, not 7 x 24 h: a week containing a DST transition is not 168
 * hours long, and every boundary here has to land on a local midnight.
 */
object WeeklyReview {

    private const val DAY_MS = 24L * 60 * 60 * 1000

    fun assemble(
        s: AdaptationSnapshot,
        weekStartMs: Long,
        sessionsTarget: Int,
        hasDeloadShadow: Boolean,
        /** The live training block (Coach v3 C), when one is running. */
        block: com.forge.app.data.db.entities.TrainingBlock? = null,
        /** The zone [weekStartMs] is a local midnight in — the week boundaries are calendar dates. */
        zone: ZoneId = ZoneId.systemDefault(),
        t: AdaptThresholds = AdaptThresholds()
    ): WeeklyReviewData {
        // Calendar weeks, not 7 x 24 h. weekStartMs is this Monday 00:00 local, but a week spanning a
        // DST transition is 23 or 25 hours x 7, so subtracting fixed days landed the earlier
        // boundaries an hour off a local midnight. Spring forward pushed "last week" back into Sunday
        // 23:00, double-counting a late Sunday session across two Briefs; fall back pushed it forward
        // into Monday 01:00, so a 00:20 session fell into neither window and vanished from both the
        // session count and the volume baseline.
        val weekStartDate = Instant.ofEpochMilli(weekStartMs).atZone(zone).toLocalDate()
        val lastWeekStart = weekStartDate.minusWeeks(1).atStartOfDay(zone).toInstant().toEpochMilli()
        val priorWeekStart = weekStartDate.minusWeeks(2).atStartOfDay(zone).toInstant().toEpochMilli()

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

        val lastVol = lastWeek.sumOf { it.totalVolumeLb ?: 0.0 }
        val priorVol = priorWeek.sumOf { it.totalVolumeLb ?: 0.0 }

        val focus = when {
            hasDeloadShadow -> "Recovery is the work this week. Go lighter, sleep more, and let the numbers reset."
            stalled > 0 -> "Chase the top of your rep ranges on the stalled lifts. Finishing the range is what restarts progress."
            // A real training block names its phase; otherwise a line grounded in the week's own
            // numbers, never the same generic cue every quiet week.
            // A REAL block (Coach v3 C) speaks for itself; the inferred phase copy is only the
            // fallback for users who never started one, and pure guesswork gets last say.
            else -> block?.let { BlockPlanner.describe(it) }
                ?: mesocycleFocus(s, weekStartDate, zone, t)
                ?: quietFocus(lastWeek.size, sessionsTarget, prs, lastVol, priorVol)
        }

        return WeeklyReviewData(
            sessionsLastWeek = lastWeek.size,
            sessionsTarget = sessionsTarget,
            volumeLastWeekLb = lastVol,
            volumePriorWeekLb = priorVol,
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

    /**
     * A quiet-week cue grounded in the week's own numbers — used when there's no real block to
     * name (§11: never the same generic line every calm week). Reads PRs, then the volume trend,
     * then session pace, before a last-resort double-progression reminder.
     */
    private fun quietFocus(sessions: Int, target: Int, prs: Int, lastVol: Double, priorVol: Double): String {
        val delta = if (priorVol > 0) (((lastVol - priorVol) / priorVol) * 100).toInt() else null
        return when {
            prs > 0 -> "$prs PR${if (prs == 1) "" else "s"} last week. Hold the approach and keep the ranges full."
            delta != null && delta >= 5 -> "Volume climbed $delta% on the week before. Same plan, keep filling your ranges."
            delta != null && delta <= -5 -> "Volume dipped ${-delta}%. Get the sessions in and finish every range."
            target > 0 && sessions >= target -> "Every session in last week. Fill the top of each range, then add load."
            target > 0 && sessions in 1 until target -> "${target - sessions} short of target last week. The plan works if you run it."
            else -> "A quiet week. Fill each rep range, then add weight."
        }
    }

    /**
     * Mesocycle phase cue (auto-coach Phase 1 smarts): when nothing's stalled and no deload is due,
     * the most useful thing a coach can say tracks WHERE in the training block you are. The block is
     * anchored on the last deload (a tagged session, the in-place mark, or the persisted apply
     * marker), falling back to the first session; the week-in-block then maps to accumulate → build →
     * peak → "ease off soon". Returns null where there isn't enough of a block to name a phase (below
     * the deload data gate, or week 0 of a fresh block); the caller falls back to [quietFocus].
     */
    private fun mesocycleFocus(
        s: AdaptationSnapshot,
        weekStartDate: java.time.LocalDate,
        zone: ZoneId,
        t: AdaptThresholds
    ): String? {
        if (s.sessions.size < t.deloadMinSessions) return null
        val anchor = listOfNotNull(
            s.sessions.lastOrNull { it.sessionType == SessionType.DELOAD.key || it.deloadMarkedHere }?.startedAt,
            s.prefs.lastDeloadAppliedMs,
            s.sessions.firstOrNull()?.startedAt
        ).maxOrNull() ?: return null
        // Whole calendar weeks between the anchor's date and this week's Monday. The millis division
        // truncated, so a DST week could drop one and make the mesocycle line disappear for a week.
        val anchorDate = Instant.ofEpochMilli(anchor).atZone(zone).toLocalDate()
        val weeksIn = ChronoUnit.WEEKS.between(anchorDate, weekStartDate).toInt()
        val meso = t.mesocycleWeeks
        return when {
            weeksIn < 1 -> null
            weeksIn == 1 -> "Early in a fresh block, this is accumulation. Build reps and volume, keep the weights submaximal."
            // No raw week count here: with no deload on record the anchor is the first session ever,
            // so weeksIn can span the whole training history — "a long stretch" stays honest where
            // "you're 150 weeks into this block" would not.
            weeksIn >= meso -> "A long stretch with no down week. A lighter week soon will bank the gains."
            weeksIn >= meso - 1 -> "Peak week of the block. Go after rep PRs and your heaviest clean sets."
            else -> "Mid-block: the volume is banked, so intensify. Finish the top of each range, then add load."
        }
    }
}
