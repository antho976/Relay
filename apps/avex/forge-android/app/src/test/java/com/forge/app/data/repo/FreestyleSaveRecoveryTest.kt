package com.forge.app.data.repo

import android.app.Application
import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.service.wear.WearHrIngest
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class)
class FreestyleSaveRecoveryTest {
    private val context = ApplicationProvider.getApplicationContext<Context>()
    private val db = inMemoryForgeDb()
    private val clock = Clock { 10_000L }
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

    @Test fun `retry after committed save returns same completed workout without writing another graph`() = runBlocking {
        val repo = repo()
        val id = repo.saveFreestyleDraft("draft-a", 1000) { sid ->
            val eid = repo.addExerciseToSession(sid, "bench", 0)
            repo.logSet(eid, "50", 50.0, 10)
        }
        val saved = db.sessionDao().get(id)!!
        assertNotNull(saved.finishedAt)
        assertEquals(1, saved.setCount)
        assertEquals(500.0, saved.totalVolumeLb!!, 0.0)
        val recreated = repo()
        assertEquals(id, recreated.saveFreestyleDraft("draft-a", 1000) { error("must not repeat graph") })
        assertEquals(1, db.sessionDao().allFinished().size)
        assertTrue(recreated.isFreestyleDraftSaved("draft-a"))
    }

    @Test fun `failure before internal finish rolls back identity and graph so retry can finish`() = runBlocking {
        val repo = repo()
        try {
            repo.saveFreestyleDraft("draft-b", 1000) { sid ->
                val eid = repo.addExerciseToSession(sid, "bench", 0)
                repo.logSet(eid, "50", 50.0, 10)
                error("process stopped before finish")
            }
            fail("injected failure should escape")
        } catch (_: IllegalStateException) { }
        assertNull(db.sessionDao().forDraft("draft-b"))
        assertNull(db.sessionDao().getActiveSession())
        val id = repo().saveFreestyleDraft("draft-b", 1000) { sid ->
            val eid = repo.addExerciseToSession(sid, "bench", 0)
            repo.logSet(eid, "50", 50.0, 10)
        }
        assertEquals(1, db.sessionDao().get(id)!!.setCount)
        assertNotNull(db.sessionDao().get(id)!!.finishedAt)
    }
}
