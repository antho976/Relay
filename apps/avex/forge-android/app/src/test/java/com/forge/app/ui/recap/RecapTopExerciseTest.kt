package com.forge.app.ui.recap

import com.forge.app.data.db.dao.LoggedExerciseDao.SessionExerciseRow
import com.forge.app.program.CustomExerciseDef
import com.forge.app.program.CustomExerciseRegistry
import com.forge.app.program.ExerciseLibrary
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test

/**
 * The Recap's "most trained" movement, over raw (session, exercise id, swapped name) rows.
 *
 * The old aggregate grouped on `exercise_id` alone, so a custom move (whose real name lives only
 * in `swapped_name`) was named by its humanized slug, and a slot swapped late in a session kept
 * its original id under two different names, which merged two movements into one bucket. Rows
 * are resolved to a display identity first now, then bucketed by that name, then counted by
 * distinct session.
 */
class RecapTopExerciseTest {

    private val bench = ExerciseLibrary.all.first { !it.curatedOnly }
    private val sled = CustomExerciseDef("custom-sled-push-2-0", "Sled Push 2.0", "quads")

    @Before
    fun reset() = CustomExerciseRegistry.clear()

    @After
    fun tearDown() = CustomExerciseRegistry.clear()

    @Test
    fun aCustomMoveIsNamedByItsRegisteredNameNotTheSlug() {
        CustomExerciseRegistry.put(sled)
        val rows = listOf(
            SessionExerciseRow(1, sled.id, "Sled Push 2.0"),
            SessionExerciseRow(2, sled.id, "sled push 2.0"),
            SessionExerciseRow(3, bench.id, null)
        )
        assertEquals("Sled Push 2.0", recapTopExercise(rows))
    }

    @Test
    fun anUnregisteredCustomMoveStillReadsItsSwappedName() {
        // Rows written before the registry existed: the name on the row is the identity.
        val rows = listOf(
            SessionExerciseRow(1, "custom-exercise", "!!!"),
            SessionExerciseRow(2, "custom-exercise", "!!!")
        )
        assertEquals("!!!", recapTopExercise(rows))
    }

    @Test
    fun aLateSwappedSlotCountsAsTwoMovementsNotOneBucket() {
        // Same base id, two names: the slot was relabelled after some sessions were logged.
        val rows = listOf(
            SessionExerciseRow(1, bench.id, null),
            SessionExerciseRow(2, bench.id, null),
            SessionExerciseRow(3, bench.id, "Relabelled Press"),
            SessionExerciseRow(4, bench.id, "Relabelled Press"),
            SessionExerciseRow(5, bench.id, "Relabelled Press")
        )
        assertEquals("Relabelled Press", recapTopExercise(rows))
    }

    @Test
    fun countsDistinctSessionsNotRows() {
        // Three rows of A in ONE session must not beat B in two sessions.
        val rows = listOf(
            SessionExerciseRow(1, "a", "A"),
            SessionExerciseRow(1, "a", "A"),
            SessionExerciseRow(1, "a", "A"),
            SessionExerciseRow(2, "b", "B"),
            SessionExerciseRow(3, "b", "B")
        )
        assertEquals("B", recapTopExercise(rows) { _, swapped -> swapped.orEmpty() })
    }

    @Test
    fun namesAreBucketedCaseAndWhitespaceInsensitivelyAndTiesGoToTheFirstSeen() {
        val rows = listOf(
            SessionExerciseRow(1, "x", "Sled  Push"),
            SessionExerciseRow(2, "x", "sled push"),
            SessionExerciseRow(3, "y", "Row"),
            SessionExerciseRow(4, "y", "Row")
        )
        assertEquals("Sled  Push", recapTopExercise(rows) { _, swapped -> swapped.orEmpty() })
    }

    @Test
    fun noRowsMeansNoTopExercise() {
        assertNull(recapTopExercise(emptyList()))
    }
}
