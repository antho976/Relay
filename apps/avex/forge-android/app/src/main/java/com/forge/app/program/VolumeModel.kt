package com.forge.app.program

import kotlin.math.roundToInt

/**
 * Turns a split's *structure* into concrete per-slot set counts (program-unlock Phase 4 — generator
 * intelligence). Sets are scheme-based (heavy compounds get 4, accessories 3) so per-session volume
 * stays sane; **weekly volume scales naturally with frequency** because more training days = more
 * slots for a muscle. A per-muscle **weekly cap** trims junk volume on high-frequency splits, and
 * `emphasis` finally does something — focused muscles get an extra set per slot (it was a no-op before).
 */
object VolumeModel {

    const val MIN_SETS = 2
    const val MAX_SETS = 5
    private const val EMPHASIS_BONUS_SETS = 1
    /** Cap on how far an emphasised muscle's weekly total may exceed its [weeklyCap] (junk-volume guard). */
    private const val MAX_EMPHASIS_HEADROOM = 2

    private fun baseSets(scheme: RepScheme): Int = when (scheme) {
        RepScheme.STRENGTH -> 4      // the day's heavy compound
        RepScheme.HYPERTROPHY -> 3
        RepScheme.PUMP -> 3          // accessory / isolation
    }

    /** Weekly set ceiling per muscle — beyond this is junk volume; high-frequency splits get scaled down. */
    val weeklyCap: Map<MuscleGroup, Int> = mapOf(
        MuscleGroup.CHEST to 18,
        MuscleGroup.BACK to 20,
        MuscleGroup.SHOULDERS to 16,
        MuscleGroup.REAR_DELTS to 12,
        MuscleGroup.BICEPS to 16,
        MuscleGroup.TRICEPS to 16,
        MuscleGroup.QUADS to 18,
        MuscleGroup.HAMSTRINGS to 16,
        MuscleGroup.GLUTES to 14,
        MuscleGroup.CALVES to 14,
        MuscleGroup.CORE to 12
    )

    /** Muscles whose volume the `emphasis` setting boosts. Empty/"balanced" → no boost. */
    fun emphasisFocus(emphasis: String): Set<MuscleGroup> = when (emphasis) {
        "arms-shoulders", "arms_shoulders" ->
            setOf(MuscleGroup.BICEPS, MuscleGroup.TRICEPS, MuscleGroup.SHOULDERS, MuscleGroup.REAR_DELTS)
        "upper" ->
            setOf(MuscleGroup.CHEST, MuscleGroup.BACK, MuscleGroup.SHOULDERS, MuscleGroup.BICEPS, MuscleGroup.TRICEPS)
        "legs" ->
            setOf(MuscleGroup.QUADS, MuscleGroup.HAMSTRINGS, MuscleGroup.GLUTES, MuscleGroup.CALVES)
        else -> emptySet()
    }

    /**
     * Allocate sets to every slot: scheme base (+1 for [focus] muscles, ×[volumeFactor] for experience,
     * ±[bias] learned by the coach), then trim any muscle whose weekly total exceeds [weeklyCap].
     * Returns sets per day per slot.
     *
     * [minSets] is the per-slot floor — normally [MIN_SETS], but a deload week passes a lower floor so
     * already-light slots (e.g. a beginner's 2-set accessories) can actually drop instead of flooring
     * at the same value they'd have in a normal week.
     *
     * [bias] is the coach's net applied volume adjustment per muscle (CoachGenBias): each +1 lands on
     * the muscle's currently-smallest slot, each −1 comes off its largest — spread, not stacked —
     * applied BEFORE the weekly cap so the junk-volume guard still has the last word.
     */
    fun allocate(
        days: List<DayArchetype>,
        focus: Set<MuscleGroup> = emptySet(),
        volumeFactor: Double = 1.0,
        minSets: Int = MIN_SETS,
        bias: Map<MuscleGroup, Int> = emptyMap(),
        /**
         * Per-muscle weekly ceilings measured from THIS athlete (Coach v3 D's `PersonalProfile`),
         * overriding the population defaults in [weeklyCap] where they've been earned. Empty until
         * the profile has months of history, at which point generation stops using a number that
         * was never about this person.
         */
        personalCaps: Map<MuscleGroup, Int> = emptyMap()
    ): List<List<Int>> {
        val result: List<IntArray> = days.map { day ->
            IntArray(day.targets.size) { si ->
                val slot = day.targets[si]
                val withEmphasis = baseSets(slot.scheme) + if (slot.muscle in focus) EMPHASIS_BONUS_SETS else 0
                (withEmphasis * volumeFactor).roundToInt().coerceIn(minSets, MAX_SETS)
            }
        }
        val positions = HashMap<MuscleGroup, MutableList<Pair<Int, Int>>>()
        days.forEachIndexed { di, day ->
            day.targets.forEachIndexed { si, slot -> positions.getOrPut(slot.muscle) { mutableListOf() }.add(di to si) }
        }
        // Coach-learned volume bias: fold the net applied ±sets into the baseline.
        bias.forEach { (muscle, delta) ->
            val slots = positions[muscle] ?: return@forEach
            repeat(kotlin.math.abs(delta)) {
                if (delta > 0) {
                    val smallest = slots.filter { (di, si) -> result[di][si] < MAX_SETS }
                        .minByOrNull { (di, si) -> result[di][si] } ?: return@repeat
                    result[smallest.first][smallest.second] += 1
                } else {
                    val biggest = slots.filter { (di, si) -> result[di][si] > minSets }
                        .maxByOrNull { (di, si) -> result[di][si] } ?: return@repeat
                    result[biggest.first][biggest.second] -= 1
                }
            }
        }
        // Per-muscle weekly cap: shave the largest slots down until under the ceiling.
        positions.forEach { (muscle, slots) ->
            // Focused muscles get a little headroom above the cap so the emphasis isn't immediately
            // trimmed away — but bounded (not slots.size × bonus, which on a high-frequency split let a
            // prioritised muscle blow well past the junk-volume ceiling).
            val cap = (personalCaps[muscle] ?: weeklyCap[muscle] ?: return@forEach) +
                if (muscle in focus) minOf(slots.size * EMPHASIS_BONUS_SETS, MAX_EMPHASIS_HEADROOM) else 0
            // No slot can go below [minSets], so a muscle with enough slots has a weekly total this
            // cap cannot reach: eight slots at a floor of two is sixteen sets, whatever the ceiling
            // says. The trim loop already stopped there — it ran out of slots above the floor and
            // broke — but it did so while still "over cap", so the model went on describing a
            // ceiling the structure it had just built could never sit under.
            //
            // Trimming a muscle further means REMOVING a slot, which is a different decision from
            // resizing one and belongs to the generator that chose the split, not to this pass. So
            // state the achievable number instead of pretending: below the floor, the floor is the cap.
            val structuralFloor = slots.size * minSets
            val effectiveCap = maxOf(cap, structuralFloor)
            var total = slots.sumOf { (di, si) -> result[di][si] }
            while (total > effectiveCap) {
                val biggest = slots.filter { (di, si) -> result[di][si] > minSets }
                    .maxByOrNull { (di, si) -> result[di][si] } ?: break
                result[biggest.first][biggest.second] -= 1
                total -= 1
            }
        }
        return result.map { it.toList() }
    }
}
