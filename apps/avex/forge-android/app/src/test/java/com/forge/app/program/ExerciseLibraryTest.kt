package com.forge.app.program

import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * JVM unit tests for the unified exercise pool's swap-candidate filtering (program-unlock Phase 4 —
 * the swap picker and the generator draw from the same [ExerciseLibrary]).
 */
class ExerciseLibraryTest {

    @Test
    fun swapCandidates_dumbbellsAndBench_dropsMachineAndCable() {
        val available = setOf(Equipment.DUMBBELLS, Equipment.BENCH)
        val candidates = ExerciseLibrary.swapCandidates(MuscleGroup.CHEST, available, emptySet())
        assertTrue("should still offer alternatives", candidates.isNotEmpty())
        assertTrue(
            "never offers gear the user doesn't have",
            candidates.none { Equipment.MACHINE in it.equipment || Equipment.CABLE in it.equipment }
        )
        // Bodyweight is always allowed even though it's not in the available set.
        assertTrue("push-up stays available", candidates.any { it.id == "push-up" })
    }

    @Test
    fun swapCandidates_excludesDisliked() {
        val candidates = ExerciseLibrary.swapCandidates(
            MuscleGroup.CHEST, emptySet(), setOf("db-bench-press")
        )
        assertTrue(candidates.none { it.id == "db-bench-press" })
    }

    @Test
    fun swapCandidates_emptyEquipmentMeansAll() {
        val all = ExerciseLibrary.forMuscle(MuscleGroup.CHEST)
        val candidates = ExerciseLibrary.swapCandidates(MuscleGroup.CHEST, emptySet(), emptySet())
        assertTrue("empty equipment = no filtering", candidates.size == all.size)
    }
}
