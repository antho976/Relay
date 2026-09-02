package com.forge.app.domain.adapt

import com.forge.app.core.time.mondayStartMs
import com.forge.app.program.MuscleGroup
import kotlin.math.roundToInt

/**
 * The ONE answer to "does more weekly volume grow this muscle's strength?" — read by the cap
 * learner ([com.forge.app.domain.coach.PersonalProfile.volumeCaps], which rewrites the generated
 * program) and by the Stats display (`InsightEngine.volumeResponse`), so the two can never
 * disagree about the same history again.
 *
 * Strength change is measured WITHIN a lift before anything is aggregated. The previous
 * implementation (duplicated in both callers) stored one number per muscle-week — the maximum raw
 * e1RM across every lift of that muscle — and then subtracted consecutive weeks' maxima as if they
 * were the same exercise. Alternate a week of flat 300 lb bench + flat 50 lb fly with a week of
 * fly alone and that reads as a 250 lb loss followed by a 250 lb gain, week after week, with
 * neither lift moving. Because the bench-containing weeks were also the high-volume weeks, the
 * "swing" landed entirely in one volume tier and moved the chest cap by the full ±35% band.
 * Exercise selection was being read as physiology.
 *
 * So, per muscle:
 *  1. Bucket usable bouts by ISO week ([mondayStartMs] in the snapshot's zone). A week's VOLUME is
 *     the sum of rep sets across every lift of the muscle; each lift's STRENGTH that week is its
 *     own best working e1RM.
 *  2. For each pair of consecutive trained weeks (a → b), take every lift with a strength read in
 *     BOTH weeks and compute its percent change `(e1rm_b − e1rm_a) / e1rm_a × 100`. A lift present
 *     in only one of the two weeks says nothing about strength and is ignored. The week's
 *     "response" is the mean of those comparable per-lift changes; a pair with no comparable lift
 *     produces no response at all (rather than a fake one).
 *  3. Attribute each response to the week that DID THE WORK (week a's sets — the lagged response,
 *     so a light week never gets credit for the previous heavy week), then split the responses at
 *     the muscle's mean weekly sets into a high tier and a low tier.
 *
 * Percent rather than pounds because a 5 lb move on a 50 lb fly and on a 300 lb bench are not the
 * same event; callers gate on [MuscleResponse.gapPct] with their own dead band.
 */
object VolumeResponse {

    /**
     * One muscle's measured response, only produced once the sample gates have been met.
     *
     * @param trainedWeeks distinct ISO weeks the muscle was trained (the sample size callers rank by).
     * @param splitSets the mean weekly set count, rounded — the boundary between the two tiers.
     * @param highAvgPct mean within-lift e1RM change (%) over the week FOLLOWING a high-volume week.
     * @param lowAvgPct the same for weeks following a low-volume week.
     * @param highWeeks / [lowWeeks] how many week pairs fed each tier.
     */
    data class MuscleResponse(
        val muscle: MuscleGroup,
        val trainedWeeks: Int,
        val splitSets: Int,
        val highAvgPct: Double,
        val lowAvgPct: Double,
        val highWeeks: Int,
        val lowWeeks: Int
    ) {
        /** Positive: higher-volume weeks grew strength faster. Negative: they grew it slower. */
        val gapPct: Double get() = highAvgPct - lowAvgPct
    }

    /**
     * @param minWeeks distinct trained weeks a muscle needs before it is analysed at all.
     * @param minPerTier week pairs needed in EACH volume tier before the comparison is trusted.
     */
    fun analyse(s: AdaptationSnapshot, minWeeks: Int, minPerTier: Int): Map<MuscleGroup, MuscleResponse> {
        val slotMuscle = s.program.flatMap { it.slots }.associate { it.exerciseId to it.muscle }
        // muscle -> week -> rep sets across every lift of that muscle
        val setsByMuscleWeek = HashMap<MuscleGroup, HashMap<Long, Int>>()
        // muscle -> exercise id -> week -> that lift's best working e1RM in the week
        val e1rmByMuscleLiftWeek = HashMap<MuscleGroup, HashMap<String, HashMap<Long, Double>>>()
        s.exerciseHistory.forEach { (id, bouts) ->
            val muscle = slotMuscle[id] ?: return@forEach
            bouts.filter { it.countsForProgression && !it.skipped }.forEach { bout ->
                val week = mondayStartMs(bout.sessionStartedAt, s.zoneId)
                val weekSets = setsByMuscleWeek.getOrPut(muscle) { HashMap() }
                weekSets[week] = (weekSets[week] ?: 0) + bout.sets.count { it.isRepSet() }
                val e1rm = bout.bestE1rm() ?: return@forEach
                val liftWeeks = e1rmByMuscleLiftWeek.getOrPut(muscle) { HashMap() }.getOrPut(id) { HashMap() }
                liftWeeks[week] = maxOf(liftWeeks[week] ?: 0.0, e1rm)
            }
        }

        return setsByMuscleWeek.mapNotNull { (muscle, weekSets) ->
            if (weekSets.size < minWeeks) return@mapNotNull null
            val weeks = weekSets.keys.sorted()
            val avgSets = weeks.map { weekSets.getValue(it) }.average()
            val lifts = e1rmByMuscleLiftWeek[muscle].orEmpty().values
            // (week a's sets, mean within-lift % change from week a to week b), for each consecutive
            // pair of trained weeks that has at least one lift to compare.
            val responses = ArrayList<Pair<Int, Double>>()
            for (i in 0 until weeks.size - 1) {
                val a = weeks[i]
                val b = weeks[i + 1]
                val changes = lifts.mapNotNull { byWeek -> liftChangePct(byWeek[a], byWeek[b]) }
                if (changes.isNotEmpty()) responses.add(weekSets.getValue(a) to changes.average())
            }
            // Split at the MEAN weekly volume, not the median: for the common bimodal pattern (some low
            // weeks, some high) a strict `> median` puts the median value's entire mode into one tier
            // and can empty the other; the mean separates the two modes cleanly.
            val high = responses.filter { it.first > avgSets }.map { it.second }
            val low = responses.filter { it.first <= avgSets }.map { it.second }
            if (high.size < minPerTier || low.size < minPerTier) return@mapNotNull null
            muscle to MuscleResponse(
                muscle = muscle,
                trainedWeeks = weekSets.size,
                splitSets = avgSets.roundToInt(),
                highAvgPct = high.average(),
                lowAvgPct = low.average(),
                highWeeks = high.size,
                lowWeeks = low.size
            )
        }.toMap()
    }

    /** Percent change of ONE lift's e1RM between two weeks; null unless it was read in both. */
    private fun liftChangePct(from: Double?, to: Double?): Double? =
        if (from == null || to == null || from <= 0.0) null else (to - from) / from * 100
}
