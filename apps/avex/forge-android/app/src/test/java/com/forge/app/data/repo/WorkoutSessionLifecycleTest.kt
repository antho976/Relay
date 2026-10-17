package com.forge.app.data.repo

import android.app.Application
import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.service.wear.WearHrIngest
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The open session's lifecycle at the repository: a workout left open before a break must not
 * swallow the first one back (resuming it poured today's sets into a session dated weeks ago), and a
 * swiped-away set can be put back while its session is still open.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class)
class WorkoutSessionLifecycleTest {
    private val context = ApplicationProvider.getApplicationContext<Context>()
    private val db = inMemoryForgeDb()
    private val day = 24L * 60 * 60 * 1000
    private var now = 100 * day
    private val clock = Clock { now }
    private val settings = SettingsRepository(context, clock)
    private val custom = ProgramCustomizationRepository(db.programCustomizationDao())
    private val program = ProgramRepository(db, db.programDao(), db.programCustomizationDao(), custom,
        db.exerciseCustomizationDao(), db.sessionDao(), db.coachDao(), settings,
        dagger.Lazy { error("generation unused") }, clock, context)
    private fun repo() = WorkoutRepository(db.sessionDao(), db.loggedExerciseDao(), db.loggedSetDao(),
        db.moodDao(), db.sessionBreakDao(), db.restEventDao(), db.suggestionOutcomeDao(),
        db.sessionSegmentDao(), db.bodyweightDao(), db.sessionHrSampleDao(), HealthConnectManager(context),
        clock, settings, program, WearHrIngest(db.sessionDao(), db.sessionHrSampleDao(), clock), db, context)
    @After fun close() = db.close()

    @Test fun `a session abandoned weeks ago is saved to history and today starts fresh`() = runBlocking {
        val repo = repo()
        val startedAt = now - 21 * day
        val old = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = startedAt))
        val ex = repo.addExerciseToSession(old, "bench", 0)
        now = startedAt + 20 * 60 * 1000
        repo.logSet(ex, "100", 100.0, 5)
        val lastSetAt = now

        now = startedAt + 21 * day
        val today = repo.startOrResumeSession("upper-a")

        assertTrue(today.created)
        assertNotEquals(old, today.session.id)
        val closed = db.sessionDao().get(old)!!
        // Finished at its last set, not at the moment it was found.
        assertEquals(lastSetAt, closed.finishedAt)
        assertEquals(1, closed.setCount)
    }

    @Test fun `an empty session abandoned overnight is discarded`() = runBlocking {
        val repo = repo()
        val old = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = now))
        now += 13 * 60 * 60 * 1000
        val today = repo.startOrResumeSession("upper-a")
        assertTrue(today.created)
        assertNull(db.sessionDao().get(old))
    }

    @Test fun `a session paused for a long rest is still resumed`() = runBlocking {
        val repo = repo()
        val open = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = now))
        now += 2 * 60 * 60 * 1000
        val again = repo.startOrResumeSession("upper-a")
        assertEquals(open, again.session.id)
        assertEquals(false, again.created)
    }

    @Test fun `a deleted set comes back with its id while the session is open`() = runBlocking {
        val repo = repo()
        val open = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = now))
        val ex = repo.addExerciseToSession(open, "bench", 0)
        repo.logSet(ex, "100", 100.0, 5)
        val set = db.loggedSetDao().allForSession(open).single()
        repo.deleteSet(set)
        assertTrue(repo.restoreSet(set))
        assertEquals(listOf(set.id), db.loggedSetDao().allForSession(open).map { it.id })
    }

    @Test fun `a deleted set does not come back into a finished session`() = runBlocking {
        val repo = repo()
        val open = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = now))
        val ex = repo.addExerciseToSession(open, "bench", 0)
        repo.logSet(ex, "100", 100.0, 5)
        val set = db.loggedSetDao().allForSession(open).single()
        repo.deleteSet(set)
        db.sessionDao().finishIfUnfinished(open, now)
        assertEquals(false, repo.restoreSet(set))
    }

    @Test fun `the startup sweep closes a stale session and says what it kept`() = runBlocking {
        val repo = repo()
        val startedAt = now - 10 * day
        val old = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = startedAt))
        val ex = repo.addExerciseToSession(old, "bench", 0)
        now = startedAt + 60_000
        repo.logSet(ex, "100", 100.0, 5)
        now = startedAt + 10 * day
        val closed = repo.closeStaleSession()!!
        assertEquals("upper-a", closed.dayKey)
        assertTrue(closed.finishedToHistory)
        assertNull(db.sessionDao().getActiveSession())
        // A fresh session is left alone.
        db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = now))
        assertNull(repo.closeStaleSession())
    }

    // ── A sitting the process died in (session_segment.last_seen_at) ──────────────────────────

    @Test fun `a sitting killed mid-rest keeps the time up to its last heartbeat`() = runBlocking {
        val repo = repo()
        val startedAt = now
        val session = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = startedAt))
        val ex = repo.addExerciseToSession(session, "bench", 0)
        val seg = repo.openSessionSegment(session, startedAt)
        now = startedAt + 2 * 60_000
        repo.logSet(ex, "100", 100.0, 5)
        // Resting, screen open, then the process is killed: the last beat landed 3 min later.
        repo.touchSessionSegment(seg, startedAt + 5 * 60_000)

        now = startedAt + 40 * 60_000
        repo.closeDanglingSegments(session)

        // Five minutes banked, not the two up to the set the old rule stopped at.
        assertEquals(5 * 60_000L, repo.closedSegmentMs(session))
    }

    @Test fun `a sitting killed during the warm-up keeps it`() = runBlocking {
        val repo = repo()
        val session = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = now))
        val seg = repo.openSessionSegment(session, now)
        repo.touchSessionSegment(seg, now + 8 * 60_000)

        now += 60 * 60_000
        repo.closeDanglingSegments(session)

        assertEquals(8 * 60_000L, repo.closedSegmentMs(session))
    }

    @Test fun `a screen left open overnight does not bank the night`() = runBlocking {
        val repo = repo()
        val startedAt = now
        val session = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = startedAt))
        val ex = repo.addExerciseToSession(session, "bench", 0)
        val seg = repo.openSessionSegment(session, startedAt)
        now = startedAt + 10 * 60_000
        repo.logSet(ex, "100", 100.0, 5)
        repo.touchSessionSegment(seg, startedAt + 9 * 60 * 60_000)

        now = startedAt + 10 * 60 * 60_000
        repo.closeDanglingSegments(session)

        // The last set plus the heartbeat's trust window, not nine hours.
        assertEquals(10 * 60_000L + HEARTBEAT_TRUST_MS, repo.closedSegmentMs(session))
    }

    @Test fun `a clock set back after the last beat cannot close the sitting in the future`() = runBlocking {
        val repo = repo()
        val startedAt = now
        val session = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = startedAt))
        val seg = repo.openSessionSegment(session, startedAt)
        repo.touchSessionSegment(seg, startedAt + 20 * 60_000)

        // The process died; the clock was then corrected ten minutes back before the resume.
        now = startedAt + 10 * 60_000
        repo.closeDanglingSegments(session)

        assertEquals(10 * 60_000L, repo.closedSegmentMs(session))
    }

    @Test fun `a session closed as abandoned stops at its last set, heartbeat or not`() = runBlocking {
        val repo = repo()
        val startedAt = now
        val session = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = startedAt))
        val ex = repo.addExerciseToSession(session, "bench", 0)
        val seg = repo.openSessionSegment(session, startedAt)
        now = startedAt + 5 * 60_000
        repo.logSet(ex, "100", 100.0, 5)
        // The screen kept beating for 20 more minutes, then the process died and nobody came back.
        repo.touchSessionSegment(seg, startedAt + 25 * 60_000)

        now = startedAt + 13 * 60 * 60_000
        repo.closeStaleSession()

        val closed = db.sessionDao().get(session)!!
        assertEquals(startedAt + 5 * 60_000, closed.finishedAt)
        // Active time never runs past the finish it was stamped with.
        assertEquals(5 * 60, closed.activeSeconds)
    }

    @Test fun `a beat after the sitting was closed does not move it`() = runBlocking {
        val repo = repo()
        val session = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = now))
        val seg = repo.openSessionSegment(session, now)
        repo.closeOpenSegments(session, now + 60_000)
        repo.touchSessionSegment(seg, now + 30 * 60_000)

        repo.closeDanglingSegments(session)

        assertEquals(60_000L, repo.closedSegmentMs(session))
    }
}
