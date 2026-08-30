package com.forge.app.data.db.dao

import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.loggedExercise
import com.forge.app.data.db.loggedSet
import com.forge.app.data.db.session
import com.forge.app.data.db.types.EffortRating
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * `Session.isUntracked` states its own contract in one line: "excluded from streak, trophies,
 * suggestions". Twelve queries kept it and a dozen more did not, and the split ran straight through
 * the middle of single screens — the PRs list hides untracked rows while the lifetime PR count
 * beside it counted them, so the number and the list it was counting disagreed.
 *
 * These are the queries that answer a progression question. Every one of them gets the same
 * fixture: a tracked session and an untracked one, identical in every other respect.
 */
@RunWith(RobolectricTestRunner::class)
class TrackedSessionContractTest {

    private val db: ForgeDatabase = inMemoryForgeDb()
    private val sessions get() = db.sessionDao()
    private val exercises get() = db.loggedExerciseDao()
    private val sets get() = db.loggedSetDao()

    @After
    fun tearDown() = db.close()

    /** A finished session with one completed exercise and one set under it. */
    private suspend fun logSession(
        untracked: Boolean,
        startedAt: Long = 1_000_000L,
        exerciseId: String = "bench",
        wasPr: Boolean = false,
        difficulty: EffortRating? = null,
        swappedName: String? = null,
        hitFullTarget: Boolean = false,
        volumeLb: Double = 500.0,
        dayKey: String = "upper-a"
    ): Long {
        val sessionId = sessions.insert(
            session(startedAt = startedAt, finishedAt = startedAt + 3_600_000L, untracked = untracked, dayKey = dayKey)
                .copy(totalVolumeLb = volumeLb)
        )
        val exId = exercises.insert(
            loggedExercise(sessionId = sessionId, exerciseId = exerciseId).copy(
                wasPr = wasPr,
                difficulty = difficulty,
                swappedName = swappedName,
                hitFullTarget = hitFullTarget
            )
        )
        sets.insert(loggedSet(loggedExerciseId = exId, completedAt = startedAt))
        return sessionId
    }

    // ── Streak and session counts ─────────────────────────────────────────────

    @Test
    fun `the streak walk sees only tracked finish instants`() = runTest {
        logSession(untracked = false, startedAt = 1_000_000L)
        logSession(untracked = true, startedAt = 2_000_000L)

        assertEquals(listOf(1_000_000L + 3_600_000L), sessions.finishedAts())
        assertEquals(listOf(1_000_000L + 3_600_000L), sessions.observeFinishedAts().first())
    }

    @Test
    fun `the workouts-logged figure counts tracked sessions only`() = runTest {
        logSession(untracked = false)
        logSession(untracked = true, startedAt = 2_000_000L)

        assertEquals(1, sessions.observeFinishedCount().first())
        assertEquals(1, sessions.trackedFinishedCount())
    }

    @Test
    fun `but the does-this-install-have-history question still counts everything`() = runTest {
        // Backup warnings and the demo-data gate ask this one. Someone whose only history is
        // untracked still has history worth backing up, and must not have demo data seeded over it.
        logSession(untracked = true)
        assertEquals(1, sessions.finishedCount())
        assertEquals(0, sessions.trackedFinishedCount())
    }

    @Test
    fun `the weekly strip excludes untracked volume and counts`() = runTest {
        logSession(untracked = false, startedAt = 5_000_000L, volumeLb = 500.0)
        logSession(untracked = true, startedAt = 5_100_000L, volumeLb = 9_000.0)

        assertEquals(1, sessions.observeFinishedCountSince(4_000_000L).first())
        assertEquals(500.0, sessions.observeVolumeSince(4_000_000L).first()!!, 0.001)
    }

    @Test
    fun `the window aggregate excludes untracked sessions`() = runTest {
        logSession(untracked = false, startedAt = 5_000_000L, volumeLb = 500.0)
        logSession(untracked = true, startedAt = 5_100_000L, volumeLb = 9_000.0)

        val agg = sessions.aggregateInRange(4_000_000L, 6_000_000L)
        assertEquals(1, agg.sessionCount)
        assertEquals(500.0, agg.totalVolume!!, 0.001)
    }

    @Test
    fun `the widget agrees with the coach about what was trained today`() = runTest {
        // DirectiveRepository already filtered untracked out in Kotlin; the widget read the raw
        // query, so the two surfaces could name different next-up days on the same phone.
        logSession(untracked = true, startedAt = 5_000_000L, dayKey = "lower-a")
        assertEquals(emptyList<String>(), sessions.finishedDayKeysSince(4_000_000L))
        assertEquals(emptyList<Long>(), sessions.finishedAtsSince(4_000_000L))
    }

    @Test
    fun `distinct days trained ignores untracked days`() = runTest {
        logSession(untracked = false, dayKey = "upper-a")
        logSession(untracked = true, startedAt = 2_000_000L, dayKey = "lower-a")

        assertEquals(listOf("upper-a"), sessions.distinctDayKeysTrained())
    }

    // ── Trophy and lifetime counters ──────────────────────────────────────────

    @Test
    fun `an untracked PR does not raise the lifetime PR count`() = runTest {
        logSession(untracked = true, wasPr = true)
        assertEquals(0, exercises.prCount())

        logSession(untracked = false, startedAt = 2_000_000L, wasPr = true)
        assertEquals(1, exercises.prCount())
    }

    @Test
    fun `untracked work does not feed the ratings, swap or target tallies`() = runTest {
        logSession(
            untracked = true,
            difficulty = EffortRating.BRUTAL,
            swappedName = "Incline press",
            hitFullTarget = true
        )

        assertEquals(0, exercises.countWithRating(EffortRating.BRUTAL))
        assertEquals(0, exercises.swapCount())
        assertEquals(0, exercises.fullTargetCount())
        assertEquals(0, exercises.totalLogged())
    }

    @Test
    fun `the live session is not counted either`() = runTest {
        // The same join keeps an unfinished session out, matching every maximum in LoggedSetDao: a
        // mid-workout typo must not reach a trophy before the workout is even over.
        val open = sessions.insert(session(finishedAt = null))
        exercises.insert(loggedExercise(sessionId = open, exerciseId = "bench").copy(wasPr = true))

        assertEquals(0, exercises.prCount())
        assertEquals(0, exercises.totalLogged())
    }

    // ── The false-PR gate (finding 12) ────────────────────────────────────────

    @Test
    fun `untracked history is not history for the first-time PR gate`() = runTest {
        // The gate and the frontier it guards must agree. They did not: with untracked history only,
        // the gate said "you have history" while the frontier came back EMPTY — and an empty
        // frontier means anything beats it, so the first tracked set was painted gold at a weight
        // the lifter had already exceeded.
        logSession(untracked = true, exerciseId = "bench")

        assertFalse(sets.hasHistoryForExercise("bench", excludeLoggedExerciseId = -1L))
        assertTrue(sets.repMaxFrontierForExercise("bench", excludeLoggedExerciseId = -1L).isEmpty())
    }

    @Test
    fun `tracked history is still history, including bodyweight and assisted sets`() = runTest {
        // The gate is deliberately wider than the frontier in the other direction: a bodyweight or
        // assisted-only past is not a first-ever time, even though it contributes no frontier.
        val sessionId = sessions.insert(session())
        val exId = exercises.insert(loggedExercise(sessionId = sessionId, exerciseId = "pullup"))
        sets.insert(loggedSet(loggedExerciseId = exId, weightLb = null, assisted = true))

        assertTrue(sets.hasHistoryForExercise("pullup", excludeLoggedExerciseId = -1L))
        assertTrue(sets.repMaxFrontierForExercise("pullup", excludeLoggedExerciseId = -1L).isEmpty())
    }

    // ── Last performance (finding 10) ─────────────────────────────────────────

    @Test
    fun `last performance skips untracked sessions`() = runTest {
        logSession(untracked = false, startedAt = 1_000_000L, exerciseId = "bench")
        val friendsGym = logSession(untracked = true, startedAt = 9_000_000L, exerciseId = "bench")

        val last = exercises.lastLoggedBefore("bench", excludeSessionId = -1L)
        assertEquals("the newer untracked session must not become 'last time'", 1_000_000L,
            sessions.get(last!!.sessionId)!!.startedAt)
        assertTrue(friendsGym > 0)
    }

    @Test
    fun `last performance skips a row with no sets under it`() = runTest {
        // A swap or a skip creates the row eagerly and may never log a set into it. That empty row
        // is newer than the real work below it, so it won the ordering and the card read
        // "First time" for a lift with history — and the wrist prefilled from nothing.
        logSession(untracked = false, startedAt = 1_000_000L, exerciseId = "bench")
        val laterEmpty = sessions.insert(session(startedAt = 9_000_000L, finishedAt = 9_100_000L))
        exercises.insert(loggedExercise(sessionId = laterEmpty, exerciseId = "bench", skipped = true))

        val last = exercises.lastLoggedBefore("bench", excludeSessionId = -1L)
        assertEquals(1_000_000L, sessions.get(last!!.sessionId)!!.startedAt)
    }

    @Test
    fun `last performance skips a session still in progress`() = runTest {
        logSession(untracked = false, startedAt = 1_000_000L, exerciseId = "bench")
        val open = sessions.insert(session(startedAt = 9_000_000L, finishedAt = null))
        val openEx = exercises.insert(loggedExercise(sessionId = open, exerciseId = "bench"))
        sets.insert(loggedSet(loggedExerciseId = openEx))

        val last = exercises.lastLoggedBefore("bench", excludeSessionId = -1L)
        assertEquals(1_000_000L, sessions.get(last!!.sessionId)!!.startedAt)
    }

    @Test
    fun `with nothing but untracked history there is no last performance at all`() = runTest {
        logSession(untracked = true, exerciseId = "bench")
        assertNull(exercises.lastLoggedBefore("bench", excludeSessionId = -1L))
    }
}
