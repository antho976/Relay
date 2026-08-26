package com.forge.app.data.db.dao

import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.loggedExercise
import com.forge.app.data.db.loggedSet
import com.forge.app.data.db.session
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
 * The strength-maximum queries, which had no tests despite being where every "personal best",
 * trophy and goal figure in the app comes from.
 *
 * Twelve of these queries repeat the same exclusion contract in hand-written SQL:
 *
 *   sess.is_untracked = 0        a session logged at a friend's gym is kept out of stats entirely
 *   sess.finished_at IS NOT NULL a mid-workout typo (2255 for 225) must not reach a trophy
 *   s.is_assisted = 0            a machine-assisted rep is not a personal best
 *   s.duration_seconds IS NULL   a weighted plank is not a strength best
 *
 * Every clause is a fix for a shipped bug, and each is written out again in each query. Dropping
 * one from a single query puts two screens into disagreement about the same lift — silently, and
 * only for the users whose history happens to contain the excluded row. So the point of this suite
 * is not any one query: it is that the family cannot drift apart.
 */
@RunWith(RobolectricTestRunner::class)
class LoggedSetDaoTest {

    private val db: ForgeDatabase = inMemoryForgeDb()
    private val sets get() = db.loggedSetDao()

    @After
    fun tearDown() = db.close()

    /** Inserts a session + one exercise + one set, and returns the loggedExercise id. */
    private suspend fun log(
        exerciseId: String = "bench",
        weightLb: Double? = 100.0,
        reps: Int = 5,
        startedAt: Long = 1_000_000L,
        finished: Boolean = true,
        untracked: Boolean = false,
        assisted: Boolean = false,
        durationSeconds: Int? = null,
        skipped: Boolean = false
    ): Long {
        val sessionId = db.sessionDao().insert(
            session(
                startedAt = startedAt,
                finishedAt = if (finished) startedAt + 3_600_000L else null,
                untracked = untracked
            )
        )
        val exId = db.loggedExerciseDao().insert(
            loggedExercise(sessionId = sessionId, exerciseId = exerciseId, skipped = skipped)
        )
        sets.insert(
            loggedSet(
                loggedExerciseId = exId,
                weightLb = weightLb,
                reps = reps,
                assisted = assisted,
                durationSeconds = durationSeconds
            )
        )
        return exId
    }

    /**
     * One clean 225, and four heavier sets each disqualified by exactly ONE rule. Every query in
     * the "best ever" family must answer 225 — if any of them answers 405, that query has lost a
     * clause the others still have.
     */
    private suspend fun seedOneCleanBestAmongDisqualifiedHeavierSets() {
        log(weightLb = 225.0, reps = 3)                                  // the real best
        log(weightLb = 405.0, reps = 3, untracked = true)                // untracked session
        log(weightLb = 405.0, reps = 3, finished = false)                // still in progress
        log(weightLb = 405.0, reps = 3, assisted = true)                 // machine-assisted
        log(weightLb = 405.0, reps = 3, durationSeconds = 60)            // a weighted hold
    }

    @Test
    fun everyStrengthMaximumAppliesTheSameFourExclusions() = runTest {
        seedOneCleanBestAmongDisqualifiedHeavierSets()

        assertEquals("maxWeightForExercise", 225.0, sets.maxWeightForExercise("bench")!!, 0.001)
        assertEquals(
            "maxWeightAcrossExercises",
            225.0, sets.maxWeightAcrossExercises(listOf("bench", "squat"))!!, 0.001
        )
        assertEquals(
            "maxWeightPerExercise",
            225.0, sets.maxWeightPerExercise(listOf("bench")).single().weightLb, 0.001
        )
        assertEquals("personalBestSet", 225.0, sets.personalBestSet("bench")!!.weightLb!!, 0.001)
        assertEquals(
            "bestE1rmLbSince",
            // Epley on the clean set: 225 * (1 + 3/30). Anything higher means a disqualified set
            // reached the profile's standing figure.
            225.0 * 1.1, sets.bestE1rmLbSince(0L)!!, 0.001
        )
    }

    @Test
    fun eachExclusionIsLoadBearingOnItsOwn() = runTest {
        // The test above passes trivially if a query excluded EVERYTHING heavier for one shared
        // reason. Insert the disqualified sets one at a time and confirm each is individually the
        // thing being rejected.
        log(weightLb = 225.0, reps = 3)
        assertEquals(225.0, sets.maxWeightForExercise("bench")!!, 0.001)

        log(weightLb = 405.0, reps = 3, untracked = true)
        assertEquals("an untracked session must not raise the best", 225.0, sets.maxWeightForExercise("bench")!!, 0.001)

        log(weightLb = 415.0, reps = 3, finished = false)
        assertEquals("a live session must not raise the best", 225.0, sets.maxWeightForExercise("bench")!!, 0.001)

        log(weightLb = 425.0, reps = 3, assisted = true)
        assertEquals("an assisted set must not raise the best", 225.0, sets.maxWeightForExercise("bench")!!, 0.001)

        log(weightLb = 435.0, reps = 3, durationSeconds = 60)
        assertEquals("a timed hold must not raise the best", 225.0, sets.maxWeightForExercise("bench")!!, 0.001)
    }

    @Test
    fun aBodyweightSetHasNoWeightAndCannotBeAMaximum() = runTest {
        log(weightLb = null, reps = 20)
        assertNull("no weighted set logged yet", sets.maxWeightForExercise("bench"))
        assertNull(sets.personalBestSet("bench"))
        assertTrue("maxWeightPerExercise omits it rather than reporting zero", sets.maxWeightPerExercise(listOf("bench")).isEmpty())
    }

    @Test
    fun skippedExercisesAreExcludedFromTheE1rmPopulation() = runTest {
        // bestE1rmLbSince carries a fifth clause the max-weight queries do not — le.skipped = 0 —
        // because it feeds the adaptation engine, whose population excludes skipped work.
        log(weightLb = 225.0, reps = 3)
        log(weightLb = 405.0, reps = 3, skipped = true)
        assertEquals(225.0 * 1.1, sets.bestE1rmLbSince(0L)!!, 0.001)
        // The max-weight family deliberately does NOT filter on skipped: the set was still lifted.
        assertEquals(405.0, sets.maxWeightForExercise("bench")!!, 0.001)
    }

    @Test
    fun bestE1rmSinceHonoursItsWindow() = runTest {
        log(weightLb = 405.0, reps = 3, startedAt = 1_000_000L)
        log(weightLb = 225.0, reps = 3, startedAt = 9_000_000L)
        assertEquals("inside the window, the older heavier lift wins", 405.0 * 1.1, sets.bestE1rmLbSince(0L)!!, 0.001)
        assertEquals("outside it, only the recent lift counts", 225.0 * 1.1, sets.bestE1rmLbSince(5_000_000L)!!, 0.001)
    }

    @Test
    fun aSingleReadsAsItsOwnWeightNotAnInflatedE1rm() = runTest {
        // Mirrors E1rm.epley's reps <= 1 branch. In SQL rather than Kotlin, so the two can drift.
        log(weightLb = 315.0, reps = 1)
        assertEquals(315.0, sets.bestE1rmLbSince(0L)!!, 0.001)
    }

    // ── PR detection reads the LIVE session on purpose ──────────────────────────────────────────

    @Test
    fun theRepMaxFrontierDeliberatelyIncludesTheUnfinishedSession() = runTest {
        // The one place the finished_at rule is correctly absent. PrDetector runs DURING a workout
        // and compares the set just logged against the sets before it — including earlier sets in
        // the session still in progress. Adding finished_at here "for consistency" would make the
        // first PR of every session a false positive.
        val liveExercise = log(weightLb = 185.0, reps = 5, finished = false)
        val frontier = sets.repMaxFrontierForExercise("bench", excludeLoggedExerciseId = -1L)
        assertEquals(1, frontier.size)
        assertEquals(185.0, frontier.single().weightLb, 0.001)
        // ...and excluding the entry being judged leaves nothing behind, which is what makes a
        // first-ever set read as "no prior history" rather than as beating itself.
        assertTrue(sets.repMaxFrontierForExercise("bench", excludeLoggedExerciseId = liveExercise).isEmpty())
    }

    @Test
    fun theRepMaxFrontierStillDropsUntrackedAssistedAndTimedSets() = runTest {
        // The other three clauses DO apply here — only finished_at is different.
        log(weightLb = 225.0, reps = 3)
        log(weightLb = 405.0, reps = 3, untracked = true)
        log(weightLb = 405.0, reps = 3, assisted = true)
        log(weightLb = 405.0, reps = 3, durationSeconds = 60)
        val best = sets.repMaxFrontierForExercise("bench", excludeLoggedExerciseId = -1L)
            .maxOf { it.weightLb }
        assertEquals(225.0, best, 0.001)
    }

    @Test
    fun theFrontierKeepsTheBestWeightPerRepCount() = runTest {
        // The Pareto shape the PR check depends on: a heavy triple and a lighter set of ten are
        // both on the frontier, and only the heavier entry at a given rep count survives.
        log(weightLb = 225.0, reps = 3)
        log(weightLb = 205.0, reps = 3)
        log(weightLb = 135.0, reps = 10)
        val byReps = sets.repMaxFrontierForExercise("bench", excludeLoggedExerciseId = -1L)
            .associate { it.reps to it.weightLb }
        assertEquals(mapOf(3 to 225.0, 10 to 135.0), byReps)
    }

    @Test
    fun theHistoricalFrontierOnlySeesSessionsThatStartedEarlier() = runTest {
        // Session detail flags an e1RM as a best AT THE TIME, so a later heavier lift must not
        // retroactively un-flag an older session's PR.
        log(weightLb = 185.0, reps = 5, startedAt = 1_000L)
        log(weightLb = 405.0, reps = 5, startedAt = 9_000L)

        val before = sets.repMaxFrontierBeforeSession(listOf("bench"), beforeStartedAt = 5_000L)
        assertEquals(185.0, before.single().weightLb, 0.001)

        assertTrue(
            "an empty exercise list returns nothing rather than everything",
            sets.repMaxFrontierBeforeSession(emptyList(), beforeStartedAt = 99_000L).isEmpty()
        )
    }

    @Test
    fun theFirstEverGateSeesBodyweightAndAssistedHistoryTheFrontierIgnores() = runTest {
        // hasHistoryForExercise exists precisely because the weighted frontier cannot answer
        // "has this lift ever been done" — a lifter who has only ever done assisted or bodyweight
        // reps has history, but no frontier.
        val onlyEntry = log(weightLb = null, reps = 12, assisted = true)
        assertTrue(sets.repMaxFrontierForExercise("bench", excludeLoggedExerciseId = -1L).isEmpty())
        assertTrue(sets.hasHistoryForExercise("bench", excludeLoggedExerciseId = -1L))
        assertFalse(
            "excluding the only entry leaves no history — a genuine first-ever set",
            sets.hasHistoryForExercise("bench", excludeLoggedExerciseId = onlyEntry)
        )
    }

    // ── personalBestSet ordering ────────────────────────────────────────────────────────────────

    @Test
    fun thePersonalBestIsHeaviestThenMostRepsAtThatWeight() = runTest {
        val sessionId = db.sessionDao().insert(session())
        val exId = db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId))
        sets.insert(loggedSet(loggedExerciseId = exId, setIndex = 0, weightLb = 225.0, reps = 3))
        sets.insert(loggedSet(loggedExerciseId = exId, setIndex = 1, weightLb = 225.0, reps = 6))
        sets.insert(loggedSet(loggedExerciseId = exId, setIndex = 2, weightLb = 215.0, reps = 12))

        val best = sets.personalBestSet("bench")!!
        assertEquals(225.0, best.weightLb!!, 0.001)
        assertEquals("the heavier weight wins, then the most reps at it", 6, best.reps)
    }

    @Test
    fun theLongestHoldIsTrackedSeparatelyFromWeight() = runTest {
        val exId = log(weightLb = null, reps = 1, durationSeconds = 45)
        sets.insert(loggedSet(loggedExerciseId = exId, setIndex = 1, weightLb = null, reps = 1, durationSeconds = 70))
        assertEquals(70, sets.bestHoldSecondsForExercise("bench"))
        assertNull("a hold is not a weight best", sets.maxWeightForExercise("bench"))
    }

    // ── The next set index ──────────────────────────────────────────────────────────────────────

    @Test
    fun theNextSetIndexComesFromTheMaximumNotTheRowCount() = runTest {
        // The documented bug: deleting a set from the MIDDLE leaves indices 0 and 2 with a count of
        // 2, so a count-derived next index writes a second index 2. The wrist then resolves its
        // prefill with maxByOrNull { setIndex } against that tie, and the target weight it showed
        // could flip between two different sets between recompositions.
        val sessionId = db.sessionDao().insert(session())
        val exId = db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId))
        sets.insert(loggedSet(loggedExerciseId = exId, setIndex = 0))
        val middle = sets.insert(loggedSet(loggedExerciseId = exId, setIndex = 1))
        sets.insert(loggedSet(loggedExerciseId = exId, setIndex = 2))

        sets.delete(sets.get(middle)!!)

        assertEquals("two rows remain", 2, sets.countForLoggedExercise(exId))
        assertEquals("but the next index must be 3, not 2", 2, sets.maxSetIndex(exId))
    }

    @Test
    fun anExerciseWithNoSetsHasNoMaxIndex() = runTest {
        val sessionId = db.sessionDao().insert(session())
        val exId = db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId))
        assertNull("null, not 0 — 0 is a real index", sets.maxSetIndex(exId))
        assertEquals(0, sets.countForLoggedExercise(exId))
    }

    @Test
    fun setsComeBackInSetIndexOrderRegardlessOfInsertOrder() = runTest {
        val sessionId = db.sessionDao().insert(session())
        val exId = db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId))
        sets.insert(loggedSet(loggedExerciseId = exId, setIndex = 2, weightLb = 3.0))
        sets.insert(loggedSet(loggedExerciseId = exId, setIndex = 0, weightLb = 1.0))
        sets.insert(loggedSet(loggedExerciseId = exId, setIndex = 1, weightLb = 2.0))
        assertEquals(
            listOf(1.0, 2.0, 3.0),
            sets.forLoggedExercise(exId).map { it.weightLb }
        )
    }

    @Test
    fun readQueriesKeepTimedSetsThatTheAggregatesDrop() = runTest {
        // The split the KDoc states: display queries show a hold, strength maxima do not.
        val exId = log(weightLb = 45.0, reps = 1, durationSeconds = 60)
        assertEquals(1, sets.forLoggedExercise(exId).size)
        assertNull(sets.maxWeightForExercise("bench"))
    }

    // ── Referential integrity ───────────────────────────────────────────────────────────────────

    @Test
    fun deletingASessionCascadesToItsExercisesAndSets() = runTest {
        // Both foreign keys declare CASCADE. If either failed to take effect, orphaned rows would
        // stay joined to nothing — invisible on every screen, and permanently counted by any
        // aggregate that does not join through session.
        val sessionId = db.sessionDao().insert(session())
        val exId = db.loggedExerciseDao().insert(loggedExercise(sessionId = sessionId))
        val setId = sets.insert(loggedSet(loggedExerciseId = exId, weightLb = 225.0))
        assertEquals(225.0, sets.maxWeightForExercise("bench")!!, 0.001)

        db.sessionDao().delete(db.sessionDao().get(sessionId)!!)

        assertNull("the set is gone with its session", sets.get(setId))
        assertNull("and no longer counts toward any maximum", sets.maxWeightForExercise("bench"))
    }
}
