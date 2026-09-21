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

    @Test fun heavyBonusFollowsTheSetPerformedNotJustThePlan() {
        // A 4-6 prescription performed for 12 reps at a working weight is not a heavy set: 2:00, not 3:00.
        val heavyPlan = compound.copy(reps = "4-6")
        val working = PerformedSet(reps = 12, weightLb = 100.0, referenceWeightLb = 100.0)
        assertEquals(120, RestAdvisor.restSeconds(heavyPlan, null, null, performed = working).seconds)
        // Performed for 5 at the working weight: the bonus stands.
        val heavy = PerformedSet(reps = 5, weightLb = 100.0, referenceWeightLb = 100.0)
        assertEquals(180, RestAdvisor.restSeconds(heavyPlan, null, null, performed = heavy).seconds)
        // Unknown set (a plan-level estimate): the plan text decides, as before.
        assertEquals(180, RestAdvisor.restSeconds(heavyPlan, null, null).seconds)
    }

    @Test fun lightFeelerSetsAreCappedAndNeverCountedAsEvidence() {
        // 50 lb against a 135 lb working weight is a feeler set: 1:00, not the 2:00 base or a 3:00 heavy rest.
        val light = PerformedSet(reps = 5, weightLb = 50.0, referenceWeightLb = 135.0)
        val rest = RestAdvisor.restSeconds(compound.copy(reps = "4-6"), null, null, performed = light)
        assertEquals(60, rest.seconds)
        assertTrue(rest.light)
        assertTrue(rest.reason.contains("light set"))
        // Isolation light set caps the same way; a HARD rating still adds its 15s on top.
        val iso = compound.copy(tags = listOf(ExerciseTag.ISOLATION))
        assertEquals(60, RestAdvisor.restSeconds(iso, null, null, performed = light).seconds)
        assertEquals(75, RestAdvisor.restSeconds(iso, EffortRating.HARD, null, performed = light).seconds)
        // No reference weight (first time on the lift) or bodyweight work: never judged light.
        assertFalse(RestAdvisor.restSeconds(compound, null, null, performed = PerformedSet(8, 50.0, null)).light)
        assertFalse(RestAdvisor.restSeconds(compound, null, null, performed = PerformedSet(8, null, 135.0)).light)
        // Just above the threshold is a working set.
        assertEquals(120, RestAdvisor.restSeconds(compound, null, null, performed = PerformedSet(8, 85.0, 135.0)).seconds)
        // An explicit override still wins over everything.
        assertEquals(200, RestAdvisor.restSeconds(compound, null, 200, performed = light).seconds)
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
