package com.forge.app.domain.adapt

import com.forge.app.program.MuscleGroup

/**
 * System 3 of the adaptation engine: fatigue-aware session ordering. Pure + deterministic
 * (greedy with stable tie-breaks on original position — no randomness).
 *
 * Soft goal: no two consecutive exercises share a primary muscle (the second one starts
 * pre-fatigued). Hard constraints: compounds keep the early block, superset groups are
 * never split, and a priority muscle gets the freshest eligible slot in its block.
 *
 * Silent unless the day actually has ≥ [AdaptThresholds.orderingMinAdjacencies]
 * same-muscle back-to-back pairings AND the suggested order strictly reduces them —
 * a reorder that wouldn't help is noise, not advice.
 */
object OrderingAdvisor {

    /** One exercise slot as the day screen renders it, in current display order. */
    data class OrderingItem(
        val exerciseId: String,
        val muscle: MuscleGroup,
        val isCompound: Boolean,
        val supersetGroup: String? = null
    )

    /** An unsplittable run: a superset group, or a single exercise. */
    private class OrderUnit(val items: MutableList<OrderingItem>, val minIndex: Int) {
        val firstMuscle get() = items.first().muscle
        val lastMuscle get() = items.last().muscle
        val isCompound get() = items.any { it.isCompound }
        fun hasPriority(priority: Set<MuscleGroup>) = items.any { it.muscle in priority }
    }

    fun suggestOrder(
        dayKey: String,
        items: List<OrderingItem>,
        priorityMuscles: Set<MuscleGroup> = emptySet(),
        t: AdaptThresholds = AdaptThresholds()
    ): Recommendation.SessionOrder? {
        if (items.size < 3) return null

        // Superset groups collapse into one unsplittable unit (internal order preserved).
        val units = mutableListOf<OrderUnit>()
        val unitByGroup = HashMap<String, OrderUnit>()
        items.forEachIndexed { index, item ->
            val group = item.supersetGroup
            if (group == null) {
                units += OrderUnit(mutableListOf(item), index)
            } else {
                unitByGroup[group]?.items?.add(item)
                    ?: OrderUnit(mutableListOf(item), index).also { unitByGroup[group] = it; units += it }
            }
        }

        val before = adjacencies(units)
        if (before < t.orderingMinAdjacencies) return null

        // Compounds keep the early block; greedy fill inside each block: prefer a unit
        // whose muscle differs from the previous unit's, then priority muscles, then
        // original position (stable). When every candidate clashes, concede the adjacency
        // for this step rather than deadlock.
        val ordered = mutableListOf<OrderUnit>()
        var previousMuscle: MuscleGroup? = null
        for (block in listOf(units.filter { it.isCompound }, units.filter { !it.isCompound })) {
            val remaining = block.toMutableList()
            while (remaining.isNotEmpty()) {
                val nonClashing = remaining.filter { it.firstMuscle != previousMuscle }
                val pool = nonClashing.ifEmpty { remaining }
                val pick = pool.minWithOrNull(
                    compareBy({ if (it.hasPriority(priorityMuscles)) 0 else 1 }, { it.minIndex })
                )!!
                ordered += pick
                remaining -= pick
                previousMuscle = pick.lastMuscle
            }
        }

        val after = adjacencies(ordered)
        val orderedIds = ordered.flatMap { unit -> unit.items.map { it.exerciseId } }
        if (orderedIds == items.map { it.exerciseId }) return null
        if (after >= before) return null

        val fixed = before - after
        return Recommendation.SessionOrder(
            dayKey = dayKey,
            orderedExerciseIds = orderedIds,
            adjacenciesBefore = before,
            adjacenciesAfter = after,
            reason = "Spaces out same-muscle work ($before back-to-back pairing${if (before == 1) "" else "s"} → $after) " +
                "so the second exercise isn't pre-fatigued · compounds stay early",
            confidence = if (fixed >= t.orderingHighConfidenceFix) Confidence.HIGH else Confidence.MEDIUM
        )
    }

    private fun adjacencies(units: List<OrderUnit>): Int =
        units.zipWithNext().count { (a, b) -> a.lastMuscle == b.firstMuscle }
}
