package com.forge.app.domain.coach

import com.forge.app.domain.adapt.AdaptThresholds
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.bestE1rm
import com.forge.app.domain.adapt.countsForProgression
import com.forge.app.program.MuscleGroup
import com.forge.app.program.VolumeModel
import java.time.Instant
import kotlin.math.roundToInt
import kotlin.math.abs

/**
 * What the coach knows about YOU specifically (Coach v3 D) — the learning loop, closed.
 *
 * V2 computed these same estimates and displayed them as Stats trivia: your volume response, your
 * recovery spacing, your best rep ranges, your strongest time of day. None of them fed a decision.
 * Here they become the numbers the coach plans with, replacing population defaults where the
 * evidence is strong enough to justify it.
 *
 * Two safety rules, both non-negotiable:
 *  - **Hard data gates.** Below them the profile returns the default, not a guess.
 *  - **Clamped to a safety band.** A personal volume cap can move within ±[CAP_BAND] of the
 *    population default and no further, so one noisy month can't send someone to 30 sets a week.
 */
object PersonalProfile {

    private const val DAY_MS = 24L * 60 * 60 * 1000

    /** How far a personal weekly cap may move from the population default, as a fraction. */
    const val CAP_BAND = 0.35

    /** Training weeks a muscle needs before its cap is personalised at all. */
    const val MIN_WEEKS_FOR_CAP = 8

    /** Minimum gap between the two tiers' average lagged e1RM change before a muscle's cap is
     *  personalised at all. Mirrors `AdaptThresholds.insightVolumeDeltaGapLb`, which gates the
     *  same computation on the display side. Inside the band, the population default stands. */
    const val VOLUME_RESPONSE_GAP_LB = 1.0

    /** Bouts needed on a lift before its rep-range sweet spot is trusted. */
    const val MIN_BOUTS_FOR_REPS = 10

    /** Session gaps needed before a personal recovery spacing is claimed. */
    const val MIN_GAPS_FOR_SPACING = 8

    /**
     * @param volumeCaps per-muscle weekly working-set ceilings, personalised where earned.
     * @param recoveryDays typical days this athlete needs between sessions before performance holds.
     * @param sweetSpotReps per-exercise rep count where their e1RM has actually moved most.
     * @param strongestHour hour of day their lifts read best, when the split is clear.
     */
    data class Profile(
        val volumeCaps: Map<MuscleGroup, Int>,
        val recoveryDays: Int?,
        val sweetSpotReps: Map<String, Int>,
        val strongestHour: Int?
    ) {
        /** The cap to plan with — personal where earned, population default otherwise. */
        fun capFor(muscle: MuscleGroup): Int =
            volumeCaps[muscle] ?: VolumeModel.weeklyCap[muscle] ?: DEFAULT_CAP

        /** True when anything at all has been personalised — what the UI gates its section on. */
        val hasPersonalData: Boolean
            get() = volumeCaps.isNotEmpty() || recoveryDays != null || sweetSpotReps.isNotEmpty()

        companion object {
            const val DEFAULT_CAP = 16
            val DEFAULTS = Profile(emptyMap(), null, emptyMap(), null)
        }
    }

    fun build(s: AdaptationSnapshot, t: AdaptThresholds = AdaptThresholds()): Profile = Profile(
        volumeCaps = volumeCaps(s),
        recoveryDays = recoveryDays(s),
        sweetSpotReps = sweetSpotReps(s),
        strongestHour = strongestHour(s, t)
    )

    /**
     * Personal weekly volume ceilings. The question asked of the data is narrow on purpose: in the
     * weeks this muscle got MORE than its own average, did strength move more than in the weeks it
     * got less? If yes, this athlete has room above the default. If no, they are already at or past
     * their productive ceiling and the cap comes down.
     */
    private fun volumeCaps(s: AdaptationSnapshot): Map<MuscleGroup, Int> {
        val slotMuscle = s.program.flatMap { it.slots }.associate { it.exerciseId to it.muscle }
        // How many slots the current split gives each muscle — the structural floor's input.
        val slotCount = s.program.flatMap { it.slots }.groupingBy { it.muscle }.eachCount()
        // muscle -> week -> (working sets, best e1rm that week)
        val byMuscleWeek = HashMap<MuscleGroup, HashMap<Long, Pair<Int, Double>>>()
        s.exerciseHistory.forEach { (id, bouts) ->
            val muscle = slotMuscle[id] ?: return@forEach
            bouts.filter { it.countsForProgression && !it.skipped }.forEach { bout ->
                // ISO week key, so these buckets line up with every other week in the engine.
                // Dividing epoch millis by a week put the boundary on a Thursday.
                val week = com.forge.app.core.time.mondayStartMs(bout.sessionStartedAt, s.zoneId)
                val e1rm = bout.bestE1rm() ?: return@forEach
                val sets = bout.sets.count { it.durationSeconds == null && !it.isAssisted }
                val cell = byMuscleWeek.getOrPut(muscle) { HashMap() }
                val prev = cell[week]
                cell[week] = if (prev == null) sets to e1rm
                else (prev.first + sets) to maxOf(prev.second, e1rm)
            }
        }

        return byMuscleWeek.mapNotNull { (muscle, weeks) ->
            if (weeks.size < MIN_WEEKS_FOR_CAP) return@mapNotNull null
            val ordered = weeks.entries.sortedBy { it.key }.map { it.value }
            val avgSets = ordered.map { it.first }.average()
            // A week's volume against the NEXT week's strength change — the lagged response, so a
            // light week never gets credit for the previous heavy one's work.
            val deltas = ordered.zipWithNext { a, b -> a.first to (b.second - a.second) }
            val high = deltas.filter { it.first > avgSets }.map { it.second }
            val low = deltas.filter { it.first <= avgSets }.map { it.second }
            if (high.size < 3 || low.size < 3) return@mapNotNull null

            // A DEAD BAND, not a coin toss. This was a strict `>` with no minimum gap, so 0.001 lb
            // between the high-volume and low-volume tiers' average lagged e1RM change decided
            // whether the muscle's weekly ceiling was default x 1.35 or default x 0.65 — for chest,
            // 24 sets or 12, a 2x swing recomputed on every regenerate, off inputs noisy enough that
            // one heavy week landing in the other tier flips the sign. InsightEngine.volumeResponse
            // runs this same computation for DISPLAY and already refuses to speak inside the band;
            // the generation path, which actually rewrites the user's program, did not.
            val gap = high.average() - low.average()
            if (abs(gap) < VOLUME_RESPONSE_GAP_LB) return@mapNotNull null
            val default = VolumeModel.weeklyCap[muscle] ?: Profile.DEFAULT_CAP
            val target = if (gap > 0) default * (1 + CAP_BAND) else default * (1 - CAP_BAND)
            // Clamped UP to what the current split can actually produce.
            //
            // No slot goes below `VolumeModel.MIN_SETS`, so a muscle with six slots has a weekly
            // floor of twelve sets whatever this number says. `VolumeModel` already refuses to trim
            // past that floor — but this value is also what the Coach screen PRINTS, as "up to N
            // sets a week", so a cap below the floor put a promise on screen that the generator
            // beside it could not keep. Making the number achievable here fixes the reading and the
            // prescription together, rather than teaching the UI a second rule.
            val floor = slotCount[muscle]?.times(VolumeModel.MIN_SETS) ?: 0
            muscle to maxOf(target.roundToInt(), floor).coerceAtLeast(4)
        }.toMap()
    }

    /**
     * Days this athlete actually needs between sessions. Read from their own history: gaps that
     * preceded a session where volume held up, versus gaps that preceded a worse one.
     */
    private fun recoveryDays(s: AdaptationSnapshot): Int? {
        val finished = s.sessions.filter { it.finishedAt != null && !it.isUntracked }
            .sortedBy { it.startedAt }
        if (finished.size < MIN_GAPS_FOR_SPACING + 1) return null
        val pairs = finished.zipWithNext().mapNotNull { (prev, next) ->
            val gapDays = ((next.startedAt - prev.startedAt) / DAY_MS).toInt()
            val volume = next.totalVolumeLb ?: return@mapNotNull null
            if (gapDays !in 1..7 || volume <= 0) null else gapDays to volume
        }
        if (pairs.size < MIN_GAPS_FOR_SPACING) return null
        // The spacing whose sessions carried the most work, on average.
        return pairs.groupBy { it.first }
            .filterValues { it.size >= 2 }
            .maxByOrNull { (_, v) -> v.map { it.second }.average() }
            ?.key
    }

    /**
     * The rep count each lift has actually progressed best at. Buckets bouts by their working reps
     * and asks which bucket carried the most strength gain — the same question the sweet-spot
     * insight answered as trivia, now used to bias prescriptions.
     */
    private fun sweetSpotReps(s: AdaptationSnapshot): Map<String, Int> =
        s.exerciseHistory.mapNotNull { (id, bouts) ->
            val usable = bouts.filter { it.countsForProgression && !it.skipped }
            if (usable.size < MIN_BOUTS_FOR_REPS) return@mapNotNull null
            val points = usable.mapNotNull { bout ->
                val reps = bout.sets.filter { !it.isAssisted && it.durationSeconds == null }
                    .map { it.reps }.filter { it > 0 }
                val e1rm = bout.bestE1rm()
                if (reps.isEmpty() || e1rm == null) null else reps.average().roundToInt() to e1rm
            }
            if (points.size < MIN_BOUTS_FOR_REPS) return@mapNotNull null
            val gains = points.zipWithNext { a, b -> a.first to (b.second - a.second) }
            val best = gains.groupBy { it.first }
                .filterValues { it.size >= 2 }
                .maxByOrNull { (_, v) -> v.map { it.second }.average() }
                ?.key
            best?.let { id to it }
        }.toMap()

    /** The hour their lifts read strongest, when one half of the day clearly beats the other. */
    private fun strongestHour(s: AdaptationSnapshot, t: AdaptThresholds): Int? {
        val points = s.exerciseHistory.values.flatten()
            .filter { it.countsForProgression && !it.skipped }
            .mapNotNull { bout ->
                val e1rm = bout.bestE1rm() ?: return@mapNotNull null
                val hour = Instant.ofEpochMilli(bout.sessionStartedAt).atZone(s.zoneId).hour
                hour to e1rm
            }
        if (points.size < t.insightTimePerfMinBouts) return null
        val early = points.filter { it.first < t.insightTimePerfSplitHour }.map { it.second }
        val late = points.filter { it.first >= t.insightTimePerfSplitHour }.map { it.second }
        if (early.size < 4 || late.size < 4) return null
        val earlyAvg = early.average()
        val lateAvg = late.average()
        val gap = kotlin.math.abs(earlyAvg - lateAvg) / maxOf(earlyAvg, lateAvg)
        if (gap * 100 < t.insightTimePerfPct) return null
        return if (earlyAvg > lateAvg) t.insightTimePerfSplitHour - 3 else t.insightTimePerfSplitHour + 3
    }
}
