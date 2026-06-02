package com.forge.app.program

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** JVM unit tests for the goal/experience policy mapping (program-unlock "professional plans" Phase 2). */
class GoalProfilesTest {

    @Test
    fun goalShiftsRepRanges() {
        // get_stronger goes heavier, lose_weight goes higher, build_muscle keeps the scheme default.
        assertEquals("4-6", GoalProfiles.reps("get_stronger", RepScheme.STRENGTH))
        assertEquals("15-20", GoalProfiles.reps("lose_weight", RepScheme.PUMP))
        assertEquals(RepScheme.HYPERTROPHY.reps, GoalProfiles.reps("build_muscle", RepScheme.HYPERTROPHY))
        assertEquals(RepScheme.STRENGTH.reps, GoalProfiles.reps("general_fitness", RepScheme.STRENGTH))
    }

    @Test
    fun beginnerSkipsAdvancedAndDoesLessVolume() {
        assertEquals(Difficulty.INTERMEDIATE, GoalProfiles.maxDifficulty("beginner"))
        assertEquals(Difficulty.ADVANCED, GoalProfiles.maxDifficulty("advanced"))
        assertTrue(GoalProfiles.volumeFactor("beginner") < GoalProfiles.volumeFactor("intermediate"))
        assertTrue(GoalProfiles.volumeFactor("advanced") > GoalProfiles.volumeFactor("intermediate"))
    }
}
