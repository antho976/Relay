package com.forge.app.data.importer

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The duplicate guard's notion of "the same workout": what the source states, nothing this device
 * rewrites (audit 2026-09-26, 09 P2 "Import duplicate check duplicates history").
 */
class ImportIdentityTest {

    private fun set(reps: Int = 5, weightLb: Double? = 225.0, hold: Int? = null, rpe: Double? = null, warmup: Boolean = false) =
        SetIdentity.of(reps, weightLb, hold, rpe, warmup)

    private fun exercise(
        orderIndex: Int = 0,
        ids: List<String?> = listOf("bench-press"),
        names: List<String?> = listOf("Bench Press"),
        sets: List<SetIdentity> = listOf(set())
    ) = ExerciseIdentity(orderIndex, ExerciseIdentity.keysOf(ids, names), sets)

    private fun workout(vararg exercises: ExerciseIdentity) = WorkoutIdentity(exercises.toList())

    @Test
    fun theSameWorkoutMatchesItself() {
        assertTrue(workout(exercise()).sameWorkoutAs(workout(exercise())))
    }

    @Test
    fun whatTheSourceStatesAboutEachSetIsTheIdentity() {
        val base = workout(exercise())
        listOf(
            set(reps = 6) to "reps",
            set(weightLb = 230.0) to "load",
            set(weightLb = null) to "bodyweight instead of a load",
            set(hold = 60) to "a hold",
            set(rpe = 8.5) to "RPE",
            set(warmup = true) to "a warm-up"
        ).forEach { (changed, what) ->
            assertFalse(what, base.sameWorkoutAs(workout(exercise(sets = listOf(changed)))))
        }
        assertFalse("a set more", base.sameWorkoutAs(workout(exercise(sets = listOf(set(), set())))))
    }

    @Test
    fun differentMovementsWithTheSameArithmeticAreDifferentWorkouts() {
        // Bench 3x10x100 and Row 3x10x100 on one date-only midnight must both land.
        val row = exercise(ids = listOf("barbell-row"), names = listOf("Barbell Row"))
        assertFalse(workout(exercise()).sameWorkoutAs(workout(row)))
    }

    @Test
    fun aMovementTheMatcherNowResolvesStillMatchesItsOlderSyntheticId() {
        // Stored by an earlier build that did not know "Cable Fly".
        val stored = exercise(ids = listOf("ext-cable-fly"), names = listOf("Cable Fly"))
        // Incoming today: matcher resolves the catalogue id; the synthetic id is still a candidate.
        val incoming = exercise(ids = listOf("cable-fly", "ext-cable-fly"), names = listOf("Cable Fly"))
        assertTrue(workout(stored).sameWorkoutAs(workout(incoming)))
    }

    @Test
    fun aMovementIsRecognisedByNameWhenNoIdAgrees() {
        val stored = exercise(ids = listOf("cable-fly"), names = listOf(null, "Cable Fly"))
        val incoming = exercise(ids = listOf("ext-cable-fly"), names = listOf("cable fly"))
        assertTrue(workout(stored).sameWorkoutAs(workout(incoming)))
    }

    @Test
    fun anIdNeverMatchesANameThatSpellsTheSame() {
        val stored = exercise(ids = listOf("squat"), names = emptyList())
        val incoming = exercise(ids = listOf("ext-other"), names = listOf("squat"))
        assertFalse(workout(stored).sameWorkoutAs(workout(incoming)))
    }

    @Test
    fun skippedAndEmptyExercisesCarryNoWorkAndDoNotCount() {
        val skipped = exercise(orderIndex = 1, ids = listOf("dip"), names = listOf("Dip"), sets = emptyList())
        assertTrue(workout(exercise(), skipped).sameWorkoutAs(workout(exercise())))
    }

    @Test
    fun orderIsTheStoredOrderNotTheListedOne() {
        val bench = exercise(orderIndex = 0)
        val row = exercise(orderIndex = 1, ids = listOf("barbell-row"), names = listOf("Barbell Row"))
        assertTrue(workout(bench, row).sameWorkoutAs(workout(row, bench)))
        assertFalse(workout(bench, row).sameWorkoutAs(workout(bench.copy(orderIndex = 1), row.copy(orderIndex = 0))))
    }

    @Test
    fun loadIsComparedToATenthOfAPound() {
        assertTrue(workout(exercise(sets = listOf(set(weightLb = 225.0))))
            .sameWorkoutAs(workout(exercise(sets = listOf(set(weightLb = 225.00004))))))
        // A stored 0 and an absent load are both bodyweight.
        assertTrue(workout(exercise(sets = listOf(set(weightLb = 0.0))))
            .sameWorkoutAs(workout(exercise(sets = listOf(set(weightLb = null))))))
    }
}
