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

    @Test
    fun mwm989Combo_coversEveryMuscle() {
        // Antho's real setup: dumbbells + bench + MWM-989 (cable + machine stations). Every muscle
        // must have at least one option so generated plans are never starved on this combo.
        val mwm989 = setOf(Equipment.DUMBBELLS, Equipment.BENCH, Equipment.CABLE, Equipment.MACHINE)
        MuscleGroup.entries.forEach { muscle ->
            assertTrue(
                "$muscle has no exercise on the MWM-989 combo",
                ExerciseLibrary.swapCandidates(muscle, mwm989, emptySet()).isNotEmpty()
            )
        }
    }

    @Test
    fun mwm989Combo_generatesFullDaysWithNoStarvedSlots() {
        // Every lift day should fill all its slots (no muscle so thin it drops an exercise).
        val mwm989 = setOf(Equipment.DUMBBELLS, Equipment.BENCH, Equipment.CABLE, Equipment.MACHINE)
        (3..6).forEach { days ->
            val plan = ProgramGenerator.generate(GenerationParams(days), mwm989, emptySet(), emptySet(), seed = 5L)
            val template = SplitTemplates.forDays(days)
            plan.filter { !it.key.startsWith("cardio") }.forEachIndexed { i, generated ->
                assertTrue(
                    "${generated.key} dropped a slot on MWM-989 (${generated.exercises.size}/${template[i].targets.size})",
                    generated.exercises.size == template[i].targets.size
                )
            }
        }
    }
}
