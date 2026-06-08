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
     * Allocate sets to every slot: scheme base (+1 for [focus] muscles, ×[volumeFactor] for experience),
     * then trim any muscle whose weekly total exceeds [weeklyCap]. Returns sets per day per slot.
     */
    fun allocate(
        days: List<DayArchetype>,
        focus: Set<MuscleGroup> = emptySet(),
        volumeFactor: Double = 1.0
    ): List<List<Int>> {
        val result: List<IntArray> = days.map { day ->
            IntArray(day.targets.size) { si ->
                val slot = day.targets[si]
                val withEmphasis = baseSets(slot.scheme) + if (slot.muscle in focus) EMPHASIS_BONUS_SETS else 0
                (withEmphasis * volumeFactor).roundToInt().coerceIn(MIN_SETS, MAX_SETS)
            }
        }
        // Per-muscle weekly cap: shave the largest slots down until under the ceiling.
        val positions = HashMap<MuscleGroup, MutableList<Pair<Int, Int>>>()
        days.forEachIndexed { di, day ->
            day.targets.forEachIndexed { si, slot -> positions.getOrPut(slot.muscle) { mutableListOf() }.add(di to si) }
        }
        positions.forEach { (muscle, slots) ->
            // Focused muscles get extra weekly headroom equal to the emphasis bonus, so the cap
            // doesn't immediately trim away the emphasis the user asked for.
            val cap = (weeklyCap[muscle] ?: return@forEach) +
                if (muscle in focus) slots.size * EMPHASIS_BONUS_SETS else 0
            var total = slots.sumOf { (di, si) -> result[di][si] }
            while (total > cap) {
                val biggest = slots.filter { (di, si) -> result[di][si] > MIN_SETS }
                    .maxByOrNull { (di, si) -> result[di][si] } ?: break
                result[biggest.first][biggest.second] -= 1
                total -= 1
            }
        }
        return result.map { it.toList() }
    }
}
