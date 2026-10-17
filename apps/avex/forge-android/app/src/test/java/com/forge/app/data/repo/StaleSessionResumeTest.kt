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
 * A workout left open before a break must not swallow the first one back. Resuming it poured
 * today's sets into a session dated weeks ago, which the streak, the week's stats and the coach's
 * layoff read all take at its start date.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class)
class StaleSessionResumeTest {
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
}
