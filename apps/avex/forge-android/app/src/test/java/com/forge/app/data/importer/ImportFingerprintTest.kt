package com.forge.app.data.importer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

/**
 * M-03: two workouts that differ in what they MEAN must not share a fingerprint.
 *
 * The guard's job is to tell a re-import of the same workout from a corrected copy of it. It used
 * to compare reps, weight and hold time only, so every other persisted field — assisted, AMRAP,
 * failure, set type, RPE, difficulty tag, drop annotation, an exercise's note/order/skipped flag,
 * the session's own classification — was invisible, and a corrected export was discarded as a
 * duplicate of the workout it was correcting.
 */
class ImportFingerprintTest {

    private fun set(
        reps: Int = 5,
        weightLb: Double? = 225.0,
        weightText: String = "225",
        durationSeconds: Int? = null,
        rpe: Double? = null,
        isAssisted: Boolean = false,
        isAmrap: Boolean = false,
        toFailure: Boolean = false,
        setType: String? = null,
        difficultyTag: String? = null,
        dropAnnotation: String? = null,
        completedAt: Long = FINISHED_AT
    ) = FingerprintSet(
        reps, weightLb, weightText, durationSeconds, rpe,
        isAssisted, isAmrap, toFailure, setType, difficultyTag, dropAnnotation, completedAt
    )

    private fun exercise(
        orderIndex: Int = 0,
        exerciseId: String = "bench_press",
        swappedName: String? = null,
        difficulty: String? = null,
        skipped: Boolean = false,
        note: String? = null,
        sets: List<FingerprintSet> = listOf(set())
    ) = FingerprintExercise(orderIndex, exerciseId, swappedName, difficulty, skipped, note, sets)

    private fun session(
        dayKey: String = "freestyle",
        sessionType: String = "normal",
        intensity: String = "normal",
        isUntracked: Boolean = false,
        tags: String = "",
        journal: String = "",
        exercises: List<FingerprintExercise> = listOf(exercise())
    ) = FingerprintSession(
        dayKey, sessionType, intensity, isUntracked, tags, journal,
        finishedAt = FINISHED_AT, activeSeconds = ACTIVE_SEC, prCount = 0, mood = "",
        exercises = exercises
    )

    private fun assertDistinct(changed: FingerprintSession, what: String) {
        assertNotEquals("$what changes what the workout means", fingerprintOf(session()), fingerprintOf(changed))
    }

    @Test
    fun theSameWorkoutPrintsTheSameWayTwice() {
        assertEquals(fingerprintOf(session()), fingerprintOf(session()))
    }

    @Test
    fun everySemanticSetFieldIsPartOfTheIdentity() {
        assertDistinct(session(exercises = listOf(exercise(sets = listOf(set(isAssisted = true))))), "assisted")
        assertDistinct(session(exercises = listOf(exercise(sets = listOf(set(isAmrap = true))))), "AMRAP")
        assertDistinct(session(exercises = listOf(exercise(sets = listOf(set(toFailure = true))))), "to failure")
        assertDistinct(session(exercises = listOf(exercise(sets = listOf(set(setType = "warmup"))))), "set type")
        assertDistinct(session(exercises = listOf(exercise(sets = listOf(set(rpe = 8.5))))), "RPE")
        assertDistinct(session(exercises = listOf(exercise(sets = listOf(set(difficultyTag = "hard"))))), "difficulty tag")
        assertDistinct(session(exercises = listOf(exercise(sets = listOf(set(dropAnnotation = "225 → 185"))))), "drop annotation")
        assertDistinct(session(exercises = listOf(exercise(sets = listOf(set(weightText = "2 plates"))))), "weight notation")
        assertDistinct(session(exercises = listOf(exercise(sets = listOf(set(durationSeconds = 60))))), "a hold instead of reps")
    }

    @Test
    fun everySemanticExerciseAndSessionFieldIsPartOfTheIdentity() {
        assertDistinct(session(exercises = listOf(exercise(skipped = true))), "skipped")
        assertDistinct(session(exercises = listOf(exercise(note = "left side only"))), "note")
        assertDistinct(session(exercises = listOf(exercise(difficulty = "HARD"))), "difficulty")
        assertDistinct(session(exercises = listOf(exercise(swappedName = "Floor Press"))), "swapped name")
        assertDistinct(session(sessionType = "deload"), "session type")
        assertDistinct(session(intensity = "light"), "intensity")
        assertDistinct(session(isUntracked = true), "untracked")
        assertDistinct(session(dayKey = "push"), "day key")
        assertDistinct(session(tags = "travel"), "tags")
        assertDistinct(session(journal = "felt strong"), "journal")
    }

    @Test
    fun reorderingTwoExercisesIsADifferentWorkout() {
        val bench = exercise(orderIndex = 0, exerciseId = "bench_press")
        val row = exercise(orderIndex = 1, exerciseId = "barbell_row")
        val swapped = listOf(bench.copy(orderIndex = 1), row.copy(orderIndex = 0))

        assertNotEquals(
            fingerprintOf(session(exercises = listOf(bench, row))),
            fingerprintOf(session(exercises = swapped))
        )
    }

    @Test
    fun theStoredOrderIsWhatCounts_notTheOrderTheParserListedThemIn() {
        // The database reads exercises back by order_index; a file may list them any way it likes.
        val bench = exercise(orderIndex = 0, exerciseId = "bench_press")
        val row = exercise(orderIndex = 1, exerciseId = "barbell_row")

        assertEquals(
            fingerprintOf(session(exercises = listOf(bench, row))),
            fingerprintOf(session(exercises = listOf(row, bench)))
        )
    }

    @Test
    fun freeTextCannotForgeAnotherWorkoutsPrint() {
        // Length-prefixed encoding: a note that spells out the separators of the fields after it
        // must not collide with a workout that really has those values.
        val sneaky = session(exercises = listOf(exercise(note = "x~0:~0:~0:")))
        val honest = session(exercises = listOf(exercise(note = "x", swappedName = "")))

        assertNotEquals(fingerprintOf(sneaky), fingerprintOf(honest))
    }

    @Test
    fun aWeightDifferenceFinerThanAThousandthOfAPoundIsNotADifference() {
        val a = session(exercises = listOf(exercise(sets = listOf(set(weightLb = 225.0)))))
        val b = session(exercises = listOf(exercise(sets = listOf(set(weightLb = 225.00004)))))

        assertEquals(fingerprintOf(a), fingerprintOf(b))
    }

    // ── M-03: the fields the first pass still could not see ──────────────────

    /**
     * Five values the insert writes and a user can change, and the print did not cover: an export
     * corrected in one of them alone printed identically and was discarded, on the one path whose
     * promise is that it does not lose anything.
     */
    @Test
    fun theSessionsOwnTimingCountsAndMoodAreAllPartOfTheWorkout() {
        assertDistinct(session().copy(finishedAt = FINISHED_AT + 60_000), "a corrected end time")
        assertDistinct(session().copy(activeSeconds = ACTIVE_SEC + 300), "a corrected active duration")
        assertDistinct(session().copy(prCount = 1), "a re-counted PR")
        assertDistinct(session().copy(mood = "strong"), "a mood added after the fact")
    }

    @Test
    fun aSetsOwnCompletionInstantIsPartOfTheWorkout() {
        val later = session(exercises = listOf(exercise(sets = listOf(set(completedAt = FINISHED_AT + 90_000)))))
        assertDistinct(later, "a corrected per-set instant")
    }

    private companion object {
        const val FINISHED_AT = 1_767_600_000_000L
        const val ACTIVE_SEC = 3_600
    }
}
