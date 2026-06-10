package com.forge.app.program

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Planned weekly sets per muscle — the Stats actual-vs-plan baseline. */
class VolumeTargetsTest {

    private fun plan(id: String, muscle: MuscleGroup, sets: Int) = ExercisePlan(
        id = id, name = id, sets = sets, reps = "8-12", unit = ExerciseUnit.DUMBBELL,
        muscle = muscle, difficulty = Difficulty.BEGINNER, note = "", tags = emptyList()
    )

    private fun day(key: String, vararg exercises: ExercisePlan) = DayPlan(
        key = key, defaultName = key, subtitle = "", word = "", accentHex = "",
        warmup = emptyList(), exercises = exercises.toList()
    )

    @Test
    fun sumsSlotSetsPerMuscleAcrossTheWeek() {
        val days = listOf(
            day("upper", plan("bench", MuscleGroup.CHEST, 4), plan("fly", MuscleGroup.CHEST, 3), plan("row", MuscleGroup.BACK, 4)),
            day("lower", plan("squat", MuscleGroup.QUADS, 4), plan("press2", MuscleGroup.CHEST, 3))
        )
        val planned = VolumeTargets.plannedWeeklySetsByMuscle(days)
        assertEquals(10, planned[MuscleGroup.CHEST])
        assertEquals(4, planned[MuscleGroup.BACK])
        assertEquals(4, planned[MuscleGroup.QUADS])
    }

    @Test
    fun emptyProgram_emptyTargets() {
        assertTrue(VolumeTargets.plannedWeeklySetsByMuscle(emptyList()).isEmpty())
    }
}
