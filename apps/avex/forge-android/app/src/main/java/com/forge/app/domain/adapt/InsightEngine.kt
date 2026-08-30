package com.forge.app.domain.adapt

import com.forge.app.core.time.mondayStartMs
import com.forge.app.data.db.entities.durationMinutes
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
/**
 * Always-on structural ratio counts (push/pull, quad/ham) over the insight window.
 * Shared by [InsightEngine.ratioInsight] (which gates on sample size + healthy band)
 * and the Stats balance bars (which render the raw counts unconditionally).
 */
data class RatioCounts(
    val key: String,
    val labelA: String,
    val labelB: String,
    val setsA: Int,
    val setsB: Int,
    val healthyLow: Double,
    val healthyHigh: Double
) {
    val ratio: Double? get() = if (setsA > 0 && setsB > 0) setsA.toDouble() / setsB else null
    val balanced: Boolean? get() = ratio?.let { it in healthyLow..healthyHigh }
}

object InsightEngine {

    private const val DAY_MS = 24L * 60 * 60 * 1000

    private val PUSH = setOf(MuscleGroup.CHEST, MuscleGroup.SHOULDERS, MuscleGroup.TRICEPS)
    private val PULL = setOf(MuscleGroup.BACK, MuscleGroup.REAR_DELTS, MuscleGroup.BICEPS)

    /** One bout reduced to what the Tier-5 strength rules need: when, how many working sets, best e1RM. */
    private data class BoutE1rm(val startedAt: Long, val workingSets: Int, val best: Double)

    fun evaluate(s: AdaptationSnapshot, t: AdaptThresholds = AdaptThresholds()): List<Recommendation.Insight> {
        val slots = s.program.flatMap { it.slots }.associateBy { it.exerciseId }
        val ratios = balanceRatios(s, t)
        // Per-lift bout e1RM series, computed ONCE — the lagging-lift / time-of-day / volume-response
        // rules all read it instead of each re-walking exerciseHistory and re-running Epley per set.
        val boutE1rms: Map<String, List<BoutE1rm>> = s.exerciseHistory.mapValues { (_, bouts) ->
            bouts.filter { !it.skipped }.mapNotNull { b ->
                val working = b.sets.filter { it.weightLb != null && !it.isAssisted }
                if (working.isEmpty()) null
                else BoutE1rm(b.sessionStartedAt, working.size, working.maxOf { E1rm.epley(it.weightLb!!, it.reps) })
            }
        }
        return listOfNotNull(
            bestTimeOfDay(s, t),
            mostImproved(s, slots, t),
            muscleDominance(s, slots, t),
            ratioInsight(ratios.first { it.key == "pushpull" }, t, "Push/pull balance"),
            ratioInsight(ratios.first { it.key == "quadham" }, t, "Quad/ham balance"),
            mostSkipped(s, slots, t),
            repeatedSessionSwaps(s, slots, t),
            recoverySignalsBuilding(s, t),
            estimateCalibration(s, t),
            sweetSpotRepRange(s, slots, t),
            laggingLift(s, slots, boutE1rms, t),
            timeOfDayPerformance(s, boutE1rms, t),
            volumeResponse(s, slots, boutE1rms, t),
            restResponse(s, t)
        )
    }

    /** The two structural ratios as raw counts — ungated, for the Stats balance bars. */
    fun balanceRatios(s: AdaptationSnapshot, t: AdaptThresholds = AdaptThresholds()): List<RatioCounts> {
        val slots = s.program.flatMap { it.slots }.associateBy { it.exerciseId }
        return listOf(
            ratioCounts(s, slots, t, "pushpull", "push", "pull", PUSH, PULL, t.insightPushPullLow, t.insightPushPullHigh),
            ratioCounts(
                s, slots, t, "quadham", "quad", "ham",
                setOf(MuscleGroup.QUADS), setOf(MuscleGroup.HAMSTRINGS), t.insightQuadHamLow, t.insightQuadHamHigh
            )
        )
    }

    // ── Ported: best time-of-day (#40) ─────────────────────────────────────────

    /** Hour-of-day → editorial word. Shared with the Stats "When you train" card. */
    fun timeOfDayLabel(hour: Int): String = when {
        hour < 10 -> "morning"
        hour < 13 -> "late morning"
        hour < 17 -> "afternoon"
        else -> "evening"
    }

    private fun bestTimeOfDay(s: AdaptationSnapshot, t: AdaptThresholds): Recommendation.Insight? {
        val bouts = s.exerciseHistory.values.flatten()
        val totalSets = bouts.sumOf { it.sets.size }
        if (totalSets < t.insightTimeOfDayMinSets) return null
        val byHour = bouts.groupBy { Instant.ofEpochMilli(it.sessionStartedAt).atZone(s.zoneId).hour }
            .mapValues { (_, b) -> b.sumOf { it.sets.size } }
        val best = byHour.maxByOrNull { it.value } ?: return null
        return insight("timeofday", "Best time to train", "You log the most sets in the ${timeOfDayLabel(best.key)} (${best.key}:00).")
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
                // Assisted sets are excluded from every other strength read in this engine
                // (bestWorkingE1rm, E1rm, WeeklyReview.prs) and they belong out of this one too: a
                // band-assisted pull-up logged with the band's weight anchored "most improved", so
                // the insight reported a 40% gain off a change of band rather than of strength.
                .mapNotNull { b -> b.sets.filterNot { it.isAssisted }.mapNotNull { it.weightLb }.maxOrNull() }
            if (perSession.size < t.insightImprovedMinSessions) return@mapNotNull null
            val mid = perSession.size / 2
            val first = perSession.take(mid).maxOrNull() ?: return@mapNotNull null
            val last = perSession.drop(mid).maxOrNull() ?: return@mapNotNull null
            if (first <= 0) return@mapNotNull null
            Triple(name, ((last - first) / first * 100).toInt(), exerciseId)
        }.maxByOrNull { it.second } ?: return null
        if (best.second <= t.insightImprovedMinPct) return null
        return insight("improved", "Most improved", "${best.first} is up ~${best.second}% in 3 months.")
    }

    // ── Ported: weekly muscle dominance ────────────────────────────────────────

    private fun muscleDominance(
        s: AdaptationSnapshot,
        slots: Map<String, ProgramSlotSnap>,
        t: AdaptThresholds
    ): Recommendation.Insight? {
        // ISO week, like every other "this week" the user is shown.
        val weekStart = mondayStartMs(s.nowMs, s.zoneId)
        val weekBouts = s.exerciseHistory.entries.flatMap { (id, bouts) ->
            val muscle = slots[id]?.muscle ?: return@flatMap emptyList()
            bouts.filter { it.sessionStartedAt >= weekStart }.map { muscle to it }
        }
        val totalSets = weekBouts.sumOf { it.second.sets.size }
        if (totalSets < t.insightBalanceMinWeekSets) return null
        val volumeByMuscle = weekBouts
            .groupBy({ it.first }, { it.second })
            .mapValues { (_, bouts) -> bouts.sumOf { b -> com.forge.app.domain.volume.VolumeCalculator.sessionVolumeLb(b.sets) } }
        val total = volumeByMuscle.values.sum()
        if (total <= 0) return null
        val dominant = volumeByMuscle.maxByOrNull { it.value } ?: return null
        if (dominant.value / total <= t.insightBalanceDominantShare) return null
        return insight(
            "dominance", "Muscle balance",
            // DESIGN §11: join with a comma, never an em dash.
            "${dominant.key.displayName} is over ${(t.insightBalanceDominantShare * 100).roundToInt()}% of this week's volume, consider balancing."
        )
    }

    // ── Structural ratios: push/pull, quad/ham ─────────────────────────────────

    private fun ratioCounts(
        s: AdaptationSnapshot,
        slots: Map<String, ProgramSlotSnap>,
        t: AdaptThresholds,
        key: String,
        labelA: String,
        labelB: String,
        sideA: Set<MuscleGroup>,
        sideB: Set<MuscleGroup>,
        low: Double,
        high: Double
    ): RatioCounts {
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
        return RatioCounts(key, labelA, labelB, a, b, low, high)
    }

    private fun ratioInsight(rc: RatioCounts, t: AdaptThresholds, title: String): Recommendation.Insight? {
        if (rc.setsA < t.insightRatioMinSetsPerSide || rc.setsB < t.insightRatioMinSetsPerSide) return null
        if (rc.balanced != false) return null
        val skew = if (rc.setsA.toDouble() / rc.setsB > rc.healthyHigh) rc.labelA else rc.labelB
        return insight(
            rc.key, title,
            "Last ${t.insightRatioWindowDays} days: ${rc.setsA} ${rc.labelA} sets vs ${rc.setsB} ${rc.labelB}, leaning $skew-heavy."
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
            "${worst.first} was skipped ${worst.second} of the last ${worst.third} times — swap it for something you'll actually do, or drop it."
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
            "You've swapped ${worst.first} (usually to ${worst.second}) in ${worst.third} of your last ${t.insightAdherenceWindow} sessions — set it as a persistent swap, or dislike the original."
        )
    }

    // ── Under-recovery: System 5's score, just below the firing line ───────────

    private fun recoverySignalsBuilding(s: AdaptationSnapshot, t: AdaptThresholds): Recommendation.Insight? {
        val f = DeloadAdvisor.fatigue(s, t) ?: return null
        if (f.score >= t.deloadScoreThreshold || f.score < t.deloadScoreThreshold - 2) return null
        return insight(
            "recovery", "Recovery signals building",
            "Not deload territory yet, but: ${f.drivers.joinToString(" · ")}."
        )
    }

    // ── Session-estimate calibration (zero new data, immediately useful) ──────

    private fun estimateCalibration(s: AdaptationSnapshot, t: AdaptThresholds): Recommendation.Insight? {
        val worst = s.program.mapNotNull { day ->
            val durations = s.sessions
                .filter { it.dayKey == day.dayKey && it.finishedAt != null }
                // Canonical active-time-preferred duration (shared with every display surface).
                .mapNotNull { sess -> sess.durationMinutes()?.takeIf { it in 10..240 } }
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
            "${worst.first} actually runs ~${worst.second} min (estimated ~${worst.third})."
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

    // ── Sweet-spot rep range (A2): where your estimated 1RM peaks ──────────────

    private fun repBucket(reps: Int): IntRange? = when {
        reps in 1..5 -> 1..5
        reps in 6..10 -> 6..10
        reps in 11..15 -> 11..15
        reps >= 16 -> 16..999
        else -> null
    }

    private fun bucketLabel(b: IntRange): String = if (b.last >= 999) "${b.first}+" else "${b.first}-${b.last}"

    /**
     * For the most-trained weighted lift, the rep bucket whose average Epley e1RM is highest —
     * the user's "strength sweet spot". Gated: a bucket needs enough sets to score, there must be
     * ≥ 2 scored buckets, and the top must clear the runner-up by [AdaptThresholds.insightSweetSpotEdge].
     */
    private fun sweetSpotRepRange(
        s: AdaptationSnapshot,
        slots: Map<String, ProgramSlotSnap>,
        t: AdaptThresholds
    ): Recommendation.Insight? {
        data class Cand(val name: String, val label: String, val total: Int)
        val best = s.exerciseHistory.mapNotNull { (id, bouts) ->
            val name = slots[id]?.name ?: return@mapNotNull null
            val sets = bouts.filter { !it.skipped }.flatMap { it.sets }
                .filter { it.weightLb != null && !it.isAssisted && it.reps > 0 }
            val scored = sets.groupBy { repBucket(it.reps) }
                .mapNotNull { (bucket, grp) ->
                    if (bucket != null && grp.size >= t.insightSweetSpotMinSetsPerBucket)
                        Triple(bucket, grp.map { E1rm.epley(it.weightLb!!, it.reps) }.average(), grp.size)
                    else null
                }
            if (scored.size < 2) return@mapNotNull null
            val ranked = scored.sortedByDescending { it.second }
            val top = ranked[0]; val runnerUp = ranked[1]
            if (runnerUp.second <= 0 || (top.second - runnerUp.second) / runnerUp.second < t.insightSweetSpotEdge)
                return@mapNotNull null
            Cand(name, bucketLabel(top.first), scored.sumOf { it.third })
        }.maxByOrNull { it.total } ?: return null
        return insight(
            "sweetspot.${best.name}", "Your strongest rep range",
            "${best.name} hits its highest estimated 1RM at ${best.label} reps — that's your strength sweet spot."
        )
    }

    // ── Lagging lift (A6): one lift stalling while its muscle's others climb ────

    /** Estimated-1RM growth % over the window (first-half best vs second-half best), or null below the gate. */
    private fun liftGrowthPct(series: List<BoutE1rm>, since: Long, minSessions: Int): Int? {
        val perSession = series.filter { it.startedAt >= since }.map { it.best }
        if (perSession.size < minSessions) return null
        val mid = perSession.size / 2
        val first = perSession.take(mid).maxOrNull() ?: return null
        val last = perSession.drop(mid).maxOrNull() ?: return null
        if (first <= 0) return null
        return ((last - first) / first * 100).toInt()
    }

    /**
     * Within a muscle that has ≥ 2 lifts with enough history, flag a lift that has stalled while
     * another on the same muscle is climbing. Compares each lift to its OWN trend (growth %), so it
     * stays apples-to-apples across different exercises; the most pronounced gap wins.
     */
    private fun laggingLift(
        s: AdaptationSnapshot,
        slots: Map<String, ProgramSlotSnap>,
        boutE1rms: Map<String, List<BoutE1rm>>,
        t: AdaptThresholds
    ): Recommendation.Insight? {
        val since = s.nowMs - 90 * DAY_MS
        val growth = boutE1rms.mapNotNull { (id, series) ->
            val slot = slots[id] ?: return@mapNotNull null
            liftGrowthPct(series, since, t.insightImprovedMinSessions)?.let { Triple(slot.muscle, slot.name, it) }
        }
        val candidate = growth.groupBy { it.first }
            .filterValues { it.size >= 2 }
            .mapNotNull { (muscle, lifts) ->
                val grower = lifts.maxByOrNull { it.third }!!
                val laggard = lifts.minByOrNull { it.third }!!
                if (grower.second != laggard.second &&
                    grower.third >= t.insightLagGrowthPct &&
                    laggard.third <= t.insightLagFlatPct
                ) Triple(muscle, grower, laggard) to (grower.third - laggard.third) else null
            }
            .maxByOrNull { it.second }?.first ?: return null
        val (muscle, grower, laggard) = candidate
        val lagSign = if (laggard.third >= 0) "+" else ""
        return insight(
            "lagging.${laggard.second}", "A lift falling behind",
            "On ${muscle.displayName}: ${grower.second} is climbing (+${grower.third}%) but ${laggard.second} has stalled " +
                "($lagSign${laggard.third}%) — give it some focus."
        )
    }

    // ── Tier 5 · session-start-time vs performance ─────────────────────────────

    /**
     * Are you measurably stronger at one time of day? Each bout's best e1RM is normalised to its
     * OWN lift's mean (so the time effect isn't confounded by which lifts you happen to do morning
     * vs evening), and only lifts trained in BOTH windows contribute — a lift you always do at 6am
     * tells us nothing about time-of-day. Hard-gated on bouts per side; silent until the data is real.
     */
    private fun timeOfDayPerformance(
        s: AdaptationSnapshot,
        boutE1rms: Map<String, List<BoutE1rm>>,
        t: AdaptThresholds
    ): Recommendation.Insight? {
        val split = t.insightTimePerfSplitHour
        val amRatios = mutableListOf<Double>()
        val pmRatios = mutableListOf<Double>()
        boutE1rms.values.forEach { series ->
            val perBout = series.map { b ->
                (Instant.ofEpochMilli(b.startedAt).atZone(s.zoneId).hour < split) to b.best
            }
            // Need this lift trained on BOTH sides of the split, else it isolates nothing.
            if (perBout.none { it.first } || perBout.none { !it.first }) return@forEach
            val mean = perBout.map { it.second }.average()
            if (mean <= 0) return@forEach
            perBout.forEach { (isAm, best) -> (if (isAm) amRatios else pmRatios).add(best / mean) }
        }
        if (amRatios.size < t.insightTimePerfMinBouts || pmRatios.size < t.insightTimePerfMinBouts) return null
        val amMean = amRatios.average()
        val pmMean = pmRatios.average()
        // Both means are ratios of positive bests, so a zero here means every bout on that side
        // carried no load — a user who types "0" for bodyweight movements rather than "BW". The
        // division then yields NaN or Infinity, and NaN fails every comparison, so `gap * 100 <
        // threshold` was false and the insight went out reading "~0% higher".
        if (amMean <= 0.0 || pmMean <= 0.0) return null
        val strongerAm = amMean >= pmMean
        val gap = if (strongerAm) amMean / pmMean - 1 else pmMean / amMean - 1
        if (!gap.isFinite() || gap * 100 < t.insightTimePerfPct) return null
        val pct = (gap * 100).roundToInt()
        val tail = if (strongerAm) "earlier in the day — put the hard sessions in the morning."
        else "later in the day — save the hard sessions for the afternoon."
        return insight("timeofdayperf", "When you're strongest", "Your estimated 1RMs run ~$pct% higher when you train $tail")
    }

    // ── Tier 5 · volume tolerance (A1): does more weekly volume grow strength? ──

    /** ISO-week start as an epoch-day key, in the snapshot's zone. */
    private fun weekKey(ms: Long, zone: java.time.ZoneId): Long {
        val d = Instant.ofEpochMilli(ms).atZone(zone).toLocalDate()
        return d.minusDays((d.dayOfWeek.value - 1).toLong()).toEpochDay()
    }

    /**
     * Per-muscle MEV/MRV flavour: for the muscle with the most training weeks, pair each week's
     * set count with that week's e1RM CHANGE vs the prior trained week, then split weeks at the
     * muscle's median weekly volume. If higher-volume weeks grew strength materially faster it's
     * "responding to volume — room to push"; if they grew slower it reads as a volume ceiling.
     * Heavily gated (weeks of data + weeks per tier) — exactly the rule that needs a real season.
     */
    private fun volumeResponse(
        s: AdaptationSnapshot,
        slots: Map<String, ProgramSlotSnap>,
        boutE1rms: Map<String, List<BoutE1rm>>,
        t: AdaptThresholds
    ): Recommendation.Insight? {
        // muscle -> week -> (sets, bestE1rm)
        val byMuscleWeek = HashMap<MuscleGroup, HashMap<Long, Pair<Int, Double>>>()
        boutE1rms.forEach { (id, series) ->
            val muscle = slots[id]?.muscle ?: return@forEach
            series.forEach { b ->
                val wk = weekKey(b.startedAt, s.zoneId)
                val cell = byMuscleWeek.getOrPut(muscle) { HashMap() }
                val prev = cell[wk]
                cell[wk] = if (prev == null) b.workingSets to b.best
                else (prev.first + b.workingSets) to maxOf(prev.second, b.best)
            }
        }
        val candidate = byMuscleWeek.entries.mapNotNull { (muscle, weeks) ->
            if (weeks.size < t.insightVolumeMinWeeks) return@mapNotNull null
            val ordered = weeks.entries.sortedBy { it.key }.map { it.value }
            // Split at the MEAN weekly volume, not the median: for the common bimodal pattern (some low
            // weeks, some high) a strict `> median` puts the median value's entire mode into one tier and
            // can empty the other; the mean separates the two modes cleanly.
            val avgSets = ordered.map { it.first }.average()
            // Attribute each gain to the week that DID THE WORK: a week's volume vs the e1RM change over
            // the FOLLOWING week (the lagged response). Pairing it with the same week's gain would credit
            // a low-volume peak week with the work a prior high-volume week actually did.
            val deltas = ordered.zipWithNext { a, b -> a.first to (b.second - a.second) }
            val high = deltas.filter { it.first > avgSets }.map { it.second }
            val low = deltas.filter { it.first <= avgSets }.map { it.second }
            if (high.size < t.insightVolumeMinPerTier || low.size < t.insightVolumeMinPerTier) return@mapNotNull null
            Triple(muscle, Triple(high.average(), low.average(), avgSets.roundToInt()), weeks.size)
            // Deterministic pick: most weeks of data, ties broken by a stable muscle ordinal.
        }.maxWithOrNull(compareBy({ it.third }, { -it.first.ordinal })) ?: return null
        val (muscle, stats, _) = candidate
        val (highAvg, lowAvg, splitSets) = stats
        if (abs(highAvg - lowAvg) < t.insightVolumeDeltaGapLb) return null
        return if (highAvg > lowAvg) insight(
            "volumeresponse.${muscle.code}", "Responds to volume",
            "${muscle.displayName}: your higher-volume weeks (above ~$splitSets sets) added strength faster — it's responding to more volume, so there's room to push it."
        ) else insight(
            "volumeresponse.${muscle.code}", "Near its volume ceiling",
            "${muscle.displayName}: past ~$splitSets sets/week your strength stopped moving — you may be near its volume ceiling, so extra sets aren't paying off."
        )
    }

    // ── Tier 5 · recovery curve: rest gap vs performance ───────────────────────

    /**
     * How much rest gets your best work? Each session's volume is normalised to its day-type mean
     * (so leg day doesn't outweigh arm day), then split by the rest gap since the previous session.
     * Surfaces whichever spacing yields materially more normalised volume. Gated on session count.
     */
    private fun restResponse(s: AdaptationSnapshot, t: AdaptThresholds): Recommendation.Insight? {
        val dayMean = s.sessions.groupBy { it.dayKey }.mapValues { (_, ss) ->
            val vols = ss.mapNotNull { it.totalVolumeLb }.filter { it > 0 }
            if (vols.isEmpty()) 0.0 else vols.average()
        }
        val ordered = s.sessions.filter { (it.totalVolumeLb ?: 0.0) > 0 }.sortedBy { it.startedAt }
        val rows = ordered.zipWithNext { prev, cur ->
            val mean = dayMean[cur.dayKey] ?: 0.0
            if (mean <= 0) return@zipWithNext null
            val gapDays = ((cur.startedAt - prev.startedAt) / DAY_MS).toInt()
            gapDays to (cur.totalVolumeLb!! / mean)
        }.filterNotNull()
        if (rows.size < t.insightRestMinSessions) return null
        val rested = rows.filter { it.first >= t.insightRestSplitDays }.map { it.second }
        val tight = rows.filter { it.first < t.insightRestSplitDays }.map { it.second }
        if (rested.size < t.insightRestMinPerTier || tight.size < t.insightRestMinPerTier) return null
        val restedMean = rested.average()
        val tightMean = tight.average()
        val restedBetter = restedMean >= tightMean
        val gap = if (restedBetter) restedMean / tightMean - 1 else tightMean / restedMean - 1
        if (gap * 100 < t.insightRestPct) return null
        val pct = (gap * 100).roundToInt()
        return if (restedBetter) insight(
            "restresponse", "Your recovery window",
            "You move ~$pct% more volume with ${t.insightRestSplitDays}+ days' rest before a session than with less — your body rewards the extra recovery."
        ) else insight(
            "restresponse", "Your recovery window",
            "You actually move ~$pct% more volume training more often (under ${t.insightRestSplitDays} days' rest) than when you wait longer — you recover fast."
        )
    }

    private fun insight(key: String, title: String, body: String) =
        Recommendation.Insight(key = key, title = title, body = body, confidence = Confidence.MEDIUM)
}
