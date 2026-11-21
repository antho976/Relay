package com.forge.app.data.db

import com.forge.app.data.db.entities.LoggedSet
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The races. There were 1,127 tests in this repository and not one of them started a second thread.
 *
 * That is structural rather than accidental: suspending code is driven through `runTest`, which
 * runs everything on one scheduler, so a suite built that way cannot express the sentence these
 * bugs are about — "the phone and the watch both wrote". The wrist is not a second process; its
 * commands arrive at WearSyncService and are executed by SetLogUseCase in this same process,
 * against this same Room instance, by a coroutine holding none of the day screen's locks.
 *
 * So these use `runBlocking` with a real dispatcher and genuinely parallel callers. That makes them
 * probabilistic in one direction only: they pass reliably against correct code, and against the
 * unguarded read-then-write they were written for they fail nearly every run — twenty racers
 * reading the same `MAX(set_index)` is not a narrow window.
 */
@RunWith(RobolectricTestRunner::class)
class SessionWritesConcurrencyTest {

    private val db: ForgeDatabase = inMemoryForgeDb()

    @After
    fun tearDown() = db.close()

    private companion object {
        /** Enough parallelism to lose reliably without the transaction; small enough to stay fast. */
        const val RACERS = 20
    }

    @Test
    fun `concurrent callers for one slot agree on one row`() = runBlocking {
        val sessionId = db.sessionDao().insert(session(finishedAt = null))

        val ids = withContext(Dispatchers.Default) {
            (1..RACERS).map {
                async {
                    SessionWrites.ensureSlotRow(
                        db = db,
                        sessionId = sessionId,
                        slotId = "bench",
                        exerciseId = "bench",
                        orderIndex = 0
                    )
                }
            }.awaitAll()
        }

        assertEquals("every caller must be handed the SAME row", 1, ids.toSet().size)
        assertEquals(
            "and the slot must hold exactly one row",
            1,
            db.loggedExerciseDao().forSession(sessionId).size
        )
    }

    @Test
    fun `a swapped slot still resolves to one row`() = runBlocking {
        // The swap path stores slot_id separately from exercise_id, so the lookup runs through
        // COALESCE(slot_id, exercise_id) — the case a naive unique key on exercise_id would miss.
        val sessionId = db.sessionDao().insert(session(finishedAt = null))

        val ids = withContext(Dispatchers.Default) {
            (1..RACERS).map {
                async {
                    SessionWrites.ensureSlotRow(
                        db = db,
                        sessionId = sessionId,
                        slotId = "ua1",
                        exerciseId = "incline_press",
                        orderIndex = 0,
                        swappedName = "Incline press"
                    )
                }
            }.awaitAll()
        }

        assertEquals(1, ids.toSet().size)
        val rows = db.loggedExerciseDao().forSession(sessionId)
        assertEquals(1, rows.size)
        assertEquals("ua1", rows.single().effectiveSlotId)
        assertEquals("incline_press", rows.single().exerciseId)
    }

    @Test
    fun `different slots in one session still get their own rows`() = runBlocking {
        val sessionId = db.sessionDao().insert(session(finishedAt = null))
        val slots = listOf("ua1", "ua2", "ua3", "ua4")

        withContext(Dispatchers.Default) {
            slots.flatMap { slot ->
                (1..5).map {
                    async {
                        SessionWrites.ensureSlotRow(db, sessionId, slot, slot, orderIndex = 0)
                    }
                }
            }.awaitAll()
        }

        assertEquals(slots.size, db.loggedExerciseDao().forSession(sessionId).size)
    }

    @Test
    fun `concurrent set inserts never share an index`() = runBlocking {
        val sessionId = db.sessionDao().insert(session(finishedAt = null))
        val exerciseId = db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId))

        withContext(Dispatchers.Default) {
            (1..RACERS).map {
                async { SessionWrites.insertSetWithNextIndex(db, newSet(exerciseId)) }
            }.awaitAll()
        }

        val indices = db.loggedSetDao().forLoggedExercise(exerciseId).map { it.setIndex }
        assertEquals("no set may be written twice", RACERS, indices.size)
        assertEquals("and no two may share an index", RACERS, indices.toSet().size)
        assertEquals("the indices are the dense range they claim to be", (0 until RACERS).toList(), indices.sorted())
    }

    @Test
    fun `set indices stay per-exercise under concurrent load`() = runBlocking {
        val sessionId = db.sessionDao().insert(session(finishedAt = null))
        val first = db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId, exerciseId = "bench"))
        val second = db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId, exerciseId = "row", orderIndex = 1))

        withContext(Dispatchers.Default) {
            (1..RACERS).map { i ->
                async { SessionWrites.insertSetWithNextIndex(db, newSet(if (i % 2 == 0) first else second)) }
            }.awaitAll()
        }

        val half = RACERS / 2
        assertEquals((0 until half).toList(), db.loggedSetDao().forLoggedExercise(first).map { it.setIndex }.sorted())
        assertEquals((0 until half).toList(), db.loggedSetDao().forLoggedExercise(second).map { it.setIndex }.sorted())
    }

    @Test
    fun `allocation resumes past a gap left by a deletion`() = runBlocking {
        // A count would write a second index 2 here; MAX + 1 writes 3.
        val sessionId = db.sessionDao().insert(session(finishedAt = null))
        val exerciseId = db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId))
        repeat(3) { SessionWrites.insertSetWithNextIndex(db, newSet(exerciseId)) }
        val middle = db.loggedSetDao().forLoggedExercise(exerciseId)[1]
        db.loggedSetDao().delete(middle)

        SessionWrites.insertSetWithNextIndex(db, newSet(exerciseId))

        assertEquals(
            listOf(0, 2, 3),
            db.loggedSetDao().forLoggedExercise(exerciseId).map { it.setIndex }.sorted()
        )
    }

    @Test
    fun `concurrent starts agree on one active session`() = runBlocking {
        // M-07: every racer reads "no active session" before any of them inserts, unless the read
        // and the insert are one transaction. Twenty starts, one row, one id handed to all of them.
        val results = withContext(Dispatchers.Default) {
            (1..RACERS).map { i ->
                async {
                    SessionWrites.startOrResume(db, session(finishedAt = null, startedAt = 1_000_000L + i))
                }
            }.awaitAll()
        }

        assertEquals("exactly one caller created the row", 1, results.count { it.second })
        assertEquals("and every caller was handed that same row", 1, results.map { it.first.id }.toSet().size)
        assertEquals("so one session is open", 1, openSessionCount())
    }

    @Test
    fun `a start with a session already open resumes it and inserts nothing`() = runBlocking {
        val existing = db.sessionDao().insert(session(finishedAt = null))

        val (resumed, created) = SessionWrites.startOrResume(db, session(finishedAt = null, startedAt = 2_000_000L))

        assertEquals(existing, resumed.id)
        assertEquals(false, created)
        assertEquals(1, openSessionCount())
    }

    private fun openSessionCount(): Int =
        db.query("SELECT COUNT(*) FROM session WHERE finished_at IS NULL", null).use { cursor ->
            cursor.moveToFirst()
            cursor.getInt(0)
        }

    private fun newSet(loggedExerciseId: Long) = LoggedSet(
        loggedExerciseId = loggedExerciseId,
        setIndex = 0,
        weightText = "100",
        weightLb = 100.0,
        reps = 5,
        completedAt = 1_000_000L
    )
}
