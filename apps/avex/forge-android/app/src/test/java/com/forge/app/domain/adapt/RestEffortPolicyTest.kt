package com.forge.app.domain.adapt

import com.forge.app.data.db.types.EffortRating
import com.forge.app.program.*
import org.junit.Assert.*
import org.junit.Test

class RestEffortPolicyTest {
    private val compound = ExercisePlan("db-bench-press", "DB Bench Press", 3, "8-12",
        ExerciseUnit.DUMBBELL, MuscleGroup.CHEST, Difficulty.BEGINNER, "", tags = listOf(ExerciseTag.COMPOUND))

    @Test fun ordinarySetsUseNinetySecondsOrTwoMinutes() {
        assertEquals(120, RestAdvisor.restSeconds(compound, null, null).seconds)
        assertEquals(90, RestAdvisor.restSeconds(compound, EffortRating.EASY, null).seconds)
        assertEquals(135, RestAdvisor.restSeconds(compound, EffortRating.HARD, null).seconds)
        assertEquals(150, RestAdvisor.restSeconds(compound, EffortRating.BRUTAL, null).seconds)
        assertEquals(90, RestAdvisor.restSeconds(compound.copy(tags = listOf(ExerciseTag.ISOLATION)), null, null).seconds)
    }

    @Test fun heavyBonusRequiresLowRepCompoundAndNeverParsesSecondsAsReps() {
        assertEquals(180, SessionEstimate.restSeconds(compound.copy(reps = "4-6")))
        for (reps in listOf("6-10", "8-12", "5-10s", "5/leg", "AMRAP")) {
            assertEquals(reps, 120, SessionEstimate.restSeconds(compound.copy(reps = reps)))
        }
        assertEquals(90, SessionEstimate.restSeconds(compound.copy(reps = "4-6", tags = listOf(ExerciseTag.ISOLATION))))
    }

    @Test fun slowLoggingCannotInflateRestAndExplicitOverridesWin() {
        val slow = RestTuning(mapOf(MovementRole.COMPOUND to 1.4), mapOf(MovementRole.COMPOUND to 30))
        assertEquals(120, RestAdvisor.restSeconds(compound, null, null, slow).seconds)
        assertEquals(90, RestAdvisor.restSeconds(compound, EffortRating.BRUTAL, 90, slow).seconds)
        assertEquals(240, RestAdvisor.restSeconds(compound, null, null, compoundBase = 240).seconds)
    }

    @Test fun latestSetEffortOverridesExerciseRatingWithoutInventingMissingEffort() {
        assertEquals(EffortRating.BRUTAL, RestAdvisor.effortForSet(10.0, "easy", EffortRating.EASY))
        assertEquals(EffortRating.HARD, RestAdvisor.effortForSet(9.0, null, null))
        assertEquals(EffortRating.EASY, RestAdvisor.effortForSet(7.0, null, null))
        assertEquals(EffortRating.JUST_RIGHT, RestAdvisor.effortForSet(8.0, "hard", null))
        assertEquals(EffortRating.HARD, RestAdvisor.effortForSet(Double.NaN, "hard", null))
        assertNull(RestAdvisor.effortForSet(null, null, null))
    }
}
