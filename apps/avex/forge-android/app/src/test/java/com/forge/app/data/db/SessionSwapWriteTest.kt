package com.forge.app.data.db

import com.forge.app.data.db.entities.LoggedSet
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The session-swap write is all or nothing (H-11).
 *
 * It used to be half of one: once a set existed under the row the transaction correctly refused to
 * re-key `exercise_id`, then wrote the new `swapped_name` and `swapped_unit` anyway. The window for
 * that is not hypothetical — the wrist writes sets straight into Room while the phone's swap sheet
 * is open — and the result was a row that NAMED the chosen movement while every PR, progression and
 * stats query stayed keyed to the original, with later input read in the new unit under the old
 * identity. So the contract here is simple: a row with sets is untouched by a swap, in every column.
 */
@RunWith(RobolectricTestRunner::class)
class SessionSwapWriteTest {

    private val db: ForgeDatabase = inMemoryForgeDb()

    @After
    fun tearDown() = db.close()

    private suspend fun plannedRow(): Long {
        val sessionId = db.sessionDao().insert(session(finishedAt = null))
        return db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId, exerciseId = "ua1"))
    }

    private fun newSet(loggedExerciseId: Long) = LoggedSet(
        loggedExerciseId = loggedExerciseId,
        setIndex = 0,
        weightText = "100",
        weightLb = 100.0,
        reps = 5,
        completedAt = 1_000_000L
    )

    @Test
    fun `a swap on an empty row re-keys and relabels it`() = runBlocking {
        val id = plannedRow()

        val result = SessionWrites.applySessionSwap(db, id, "Incline press", "WEIGHT", "incline_press")

        assertEquals(SessionSwapResult.APPLIED, result)
        val row = db.loggedExerciseDao().get(id)!!
        assertEquals("incline_press", row.exerciseId)
        assertEquals("the original slot is stashed so the day screen still maps the row", "ua1", row.slotId)
        assertEquals("Incline press", row.swappedName)
        assertEquals("WEIGHT", row.swappedUnit)
    }

    @Test
    fun `a swap on a row with a set is refused and writes nothing`() = runBlocking {
        val id = plannedRow()
        SessionWrites.insertSetWithNextIndex(db, newSet(id))
        val before = db.loggedExerciseDao().get(id)!!

        val result = SessionWrites.applySessionSwap(db, id, "Incline press", "PLATES", "incline_press")

        assertEquals(SessionSwapResult.REFUSED_SETS_LOGGED, result)
        val after = db.loggedExerciseDao().get(id)!!
        assertEquals("every column must survive a refused swap", before, after)
        assertEquals("ua1", after.exerciseId)
        assertNull("no relabel without a re-key", after.swappedName)
        assertNull("and no unit change under the old identity", after.swappedUnit)
        assertNull(after.slotId)
    }

    @Test
    fun `a swap on a missing row reports so without inventing one`() = runBlocking {
        val result = SessionWrites.applySessionSwap(db, 999L, "Incline press", "WEIGHT", "incline_press")

        assertEquals(SessionSwapResult.NOT_FOUND, result)
        assertNull(db.loggedExerciseDao().get(999L))
    }

    @Test
    fun `swapping back to the slot's own exercise drops the slot link`() = runBlocking {
        val id = plannedRow()
        SessionWrites.applySessionSwap(db, id, "Incline press", "WEIGHT", "incline_press")

        val result = SessionWrites.applySessionSwap(db, id, null, null, "ua1")

        assertEquals(SessionSwapResult.APPLIED, result)
        val row = db.loggedExerciseDao().get(id)!!
        assertEquals("ua1", row.exerciseId)
        assertNull("slot == exercise again, so the link is cleared", row.slotId)
        assertNull(row.swappedName)
        assertNull(row.swappedUnit)
    }

    @Test
    fun `a set racing the swap never leaves a relabel without a re-key`() = runBlocking {
        // The finding's exact shape: the sheet is open, the wrist logs set one, the phone confirms.
        // Both orderings are legitimate outcomes — the swap lands first and the set is logged under
        // the swapped exercise, or the set lands first and the swap is refused whole. What must NEVER
        // exist is the third state the old code produced: the old exercise id wearing the new name.
        repeat(20) {
            val id = plannedRow()

            val swap = withContext(Dispatchers.Default) {
                val swapping = async { SessionWrites.applySessionSwap(db, id, "Incline press", "WEIGHT", "incline_press") }
                val logging = async { SessionWrites.insertSetWithNextIndex(db, newSet(id)) }
                logging.await()
                swapping.await()
            }

            val row = db.loggedExerciseDao().get(id)!!
            val rekeyed = row.exerciseId == "incline_press"
            val relabelled = row.swappedName != null || row.swappedUnit != null
            assertEquals("name/unit and identity move together or not at all", rekeyed, relabelled)
            assertEquals(
                "the result must say what happened to the row",
                if (rekeyed) SessionSwapResult.APPLIED else SessionSwapResult.REFUSED_SETS_LOGGED,
                swap
            )
            assertTrue("the set is never lost either way", db.loggedSetDao().forLoggedExercise(id).size == 1)
        }
    }
}
