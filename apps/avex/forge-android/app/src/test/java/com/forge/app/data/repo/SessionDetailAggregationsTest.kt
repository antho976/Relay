package com.forge.app.data.repo

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Pure coverage for [buildSessionMuscleSplit] — the session-detail "muscles worked" fold.
 * No DAO/DI; it only depends on the static exercise library via Program.exercise.
 */
class SessionDetailAggregationsTest {

    @Test
    fun mergesSameMuscleAndDropsUnknownIds() {
        val split = buildSessionMuscleSplit(
            listOf(
                "db-bench-press" to 3,      // CHEST
                "db-bench-press" to 2,      // CHEST again → merges to 5
                "totally-unknown-id" to 9   // no library match → dropped
            )
        )
        assertEquals("unknown id dropped, chest entries merged", 1, split.size)
        assertEquals(5, split[0].sets)
    }

    @Test
    fun sortsBusiestMuscleFirst() {
        val split = buildSessionMuscleSplit(
            listOf(
                "db-bench-press" to 2,  // CHEST = 2
                "db-row" to 5           // BACK = 5
            )
        )
        assertEquals(2, split.size)
        assertEquals(5, split[0].sets)  // BACK first (busiest)
        assertEquals(2, split[1].sets)
    }
}
