package com.forge.app.data.repo

import android.app.Application
import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.program.Program
import com.forge.app.service.wear.WearHrIngest
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Before
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
    // startOrResumeSession treats a session whose day the LOADED program lacks as a zombie, and the
    // program facade is process-wide: a class that leaves a generated 3-day split in it made every
    // "upper-a" session here a zombie, so the resume tests passed or failed by test order.
    private val original = Program.days
    @Before fun pinProgram() = Program.setActive(Program.seedDays)
    @After fun close() { Program.setActive(original); db.close() }

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

    @Test fun `a deleted set whose number was taken since comes back after the last set`() = runBlocking {
        val repo = repo()
        val open = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = now))
        val ex = repo.addExerciseToSession(open, "bench", 0)
        repo.logSet(ex, "100", 100.0, 5)
        repo.logSet(ex, "100", 100.0, 5)
        val second = db.loggedSetDao().allForSession(open).maxBy { it.setIndex }
        repo.deleteSet(second)
        // Re-logged with corrected numbers before the undo: that one takes the same position.
        repo.logSet(ex, "105", 105.0, 5)
        assertTrue(repo.restoreSet(second, "bench"))
        assertEquals(listOf(0, 1, 2), db.loggedSetDao().allForSession(open).map { it.setIndex }.sorted())
    }

    @Test fun `a deleted set does not come back under a lift swapped in since, or a skipped row`() = runBlocking {
        val repo = repo()
        val open = db.sessionDao().insert(Session(dayKey = "upper-a", startedAt = now))
        val ex = repo.addExerciseToSession(open, "bench", 0)
        repo.logSet(ex, "100", 100.0, 5)
        val set = db.loggedSetDao().allForSession(open).single()
        repo.deleteSet(set)
        val sql = db.openHelper.writableDatabase
        // Its only set gone, the row could be swapped: the bench set must not be credited to DB press.
        sql.execSQL("UPDATE logged_exercise SET exercise_id = 'db_press' WHERE id = $ex")
        assertEquals(false, repo.restoreSet(set, "bench"))
        sql.execSQL("UPDATE logged_exercise SET exercise_id = 'bench', skipped = 1 WHERE id = $ex")
        assertEquals(false, repo.restoreSet(set, "bench"))
        assertTrue(db.loggedSetDao().allForSession(open).isEmpty())
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
}
