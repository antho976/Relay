package com.forge.app.program

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** JVM unit tests for rest recommendations + session-time estimates (professional plans Phase 4). */
class SessionEstimateTest {

    private fun plan(tags: List<ExerciseTag>, reps: String, sets: Int = 3, muscle: MuscleGroup = MuscleGroup.CHEST) =
        ExercisePlan(
            id = "x", name = "X", sets = sets, reps = reps, unit = ExerciseUnit.DUMBBELL,
            muscle = muscle, difficulty = Difficulty.BEGINNER, note = "", tags = tags
        )

    @Test
    fun compoundsRestLongerThanIsolation() {
        val compound = SessionEstimate.restSeconds(plan(listOf(ExerciseTag.COMPOUND), "8-12"))
        val isolation = SessionEstimate.restSeconds(plan(listOf(ExerciseTag.ISOLATION), "12-15"))
        assertTrue("compound rest ($compound) should exceed isolation ($isolation)", compound > isolation)
    }

    @Test
    fun heavyRepsRestLongerThanLight() {
        val heavy = SessionEstimate.restSeconds(plan(listOf(ExerciseTag.COMPOUND), "4-6"))
        val light = SessionEstimate.restSeconds(plan(listOf(ExerciseTag.COMPOUND), "10-12"))
        assertTrue("heavy ($heavy) should rest longer than light ($light)", heavy > light)
    }

    @Test
    fun nonNumericRepsDoNotCrashRest() {
        // AMRAP / timed / per-leg reps have no "heavy" bump but must still return a sane rest.
        listOf("AMRAP", "30-60s", "10/leg").forEach { reps ->
            val r = SessionEstimate.restSeconds(plan(listOf(ExerciseTag.BODYWEIGHT), reps))
            assertTrue("rest for '$reps' should be positive", r > 0)
        }
    }

    @Test
    fun moreVolumeMeansLongerEstimate() {
        fun day(sets: Int) = DayPlan(
            key = "d", defaultName = "D", subtitle = "", word = "D", accentHex = "#fff",
            warmup = emptyList(),
            exercises = listOf(plan(listOf(ExerciseTag.COMPOUND), "8-12", sets = sets))
        )
        assertTrue(SessionEstimate.estimateMinutes(day(5)) > SessionEstimate.estimateMinutes(day(2)))
        assertEquals("empty day = 0 min", 0, SessionEstimate.estimateMinutes(day(0).copy(exercises = emptyList())))
    }

    @Test
    fun deloadCutsVolume() {
        fun totalSets(deload: Boolean) = ProgramGenerator.generate(
            GenerationParams(3, deload = deload), emptySet(), emptySet(), emptySet(), seed = 8L
        ).flatMap { it.exercises }.sumOf { it.sets }
        assertTrue("deload should reduce total volume", totalSets(true) < totalSets(false))
    }
}
