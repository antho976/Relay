package com.forge.app.data.repo

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.session
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.coach.BlockPhase
import com.forge.app.domain.coach.BlockPlanner
import com.forge.app.program.Equipment
import com.forge.app.program.GenerationParams
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Background regenerations never discard an open workout (audit 2026-09-26 P1 #5 follow-up).
 *
 * The unconfirmed callers checked for an open workout and then regenerated, so one started in
 * between was deleted; and a block deload held back by an open workout was never served. Built
 * over one in-memory database like [BlockRepositoryConcurrencyTest].
 */
@RunWith(RobolectricTestRunner::class)
class BackgroundRegenerationTest {

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val db: ForgeDatabase = inMemoryForgeDb()
    private val clock = Clock { NOW }
    private val settings = SettingsRepository(context, clock)
    private val customizationRepo = CustomizationRepository(db.exerciseCustomizationDao(), db.dayNameOverrideDao())
    private val programCustomizationRepo = ProgramCustomizationRepository(db.programCustomizationDao())

    private lateinit var adaptationRepository: AdaptationRepository
    private val programRepository = ProgramRepository(
        database = db,
        dao = db.programDao(),
        customizationDao = db.programCustomizationDao(),
        programCustomizationRepo = programCustomizationRepo,
        exerciseCustomizationDao = db.exerciseCustomizationDao(),
        sessionDao = db.sessionDao(),
        coachDao = db.coachDao(),
        settings = settings,
        adaptationRepositoryLazy = dagger.Lazy<AdaptationRepository> { adaptationRepository },
        clock = clock,
        context = context
    )
    private val repo: BlockRepository

    init {
        adaptationRepository = AdaptationRepository(
            sessionDao = db.sessionDao(),
            loggedExerciseDao = db.loggedExerciseDao(),
            loggedSetDao = db.loggedSetDao(),
            moodDao = db.moodDao(),
            cardioDao = db.cardioDao(),
            bodyweightDao = db.bodyweightDao(),
            checkinDao = db.checkinDao(),
            injuryDao = db.injuryRestrictionDao(),
            adviceEventDao = db.adviceEventDao(),
            vacationDao = db.vacationDao(),
            programRepository = programRepository,
            programCustomizationRepo = programCustomizationRepo,
            customizationRepo = customizationRepo,
            settingsRepository = settings,
            healthConnectManager = HealthConnectManager(context),
            clock = clock
        )
        val coachGoalRepository = CoachGoalRepository(
            coachGoalDao = db.coachGoalDao(),
            adaptationRepository = adaptationRepository,
            clock = clock
        )
        val academyRepository = AcademyRepository(
            lessonEventDao = db.lessonEventDao(),
            adaptationRepository = adaptationRepository,
            coachDao = db.coachDao(),
            clock = clock
        )
        repo = BlockRepository(
            blockDao = db.trainingBlockDao(),
            adaptationRepository = adaptationRepository,
            coachGoalRepository = coachGoalRepository,
            academyRepository = academyRepository,
            settingsRepository = settings,
            database = db,
            clock = clock
        )
    }

    @After
    fun tearDown() = db.close()

    private companion object {
        const val NOW = 1_700_000_000_000L
        const val WEEK = "2026-W10"
    }

    private suspend fun openWorkout(): Long = db.sessionDao().insert(session(startedAt = NOW - 60_000L, finishedAt = null))

    /** The DataStore outlives a test class in this JVM; start from no deload and nothing owed. */
    private suspend fun cleanDeloadState() {
        settings.setDeloadWeekStartMs(0L)
        settings.setBlockDeloadOwed(false)
    }

    private suspend fun blockInDeload() {
        db.trainingBlockDao().insert(BlockPlanner.start(nowMs = NOW, weekId = WEEK).copy(phase = BlockPhase.DELOAD.code))
    }

    @Test
    fun `an unconfirmed regenerate backs out inside its transaction when a workout is open`() = runBlocking {
        val workout = openWorkout()

        val replaced = programRepository.generate(
            GenerationParams(3), Equipment.entries.toSet(), emptySet(), emptySet(), unlessWorkoutOpen = true
        )

        assertFalse(replaced)
        assertNotNull("the workout survives", db.sessionDao().get(workout))
        assertTrue("no program was written", db.programDao().days().isEmpty())
        assertNull("and no intent is left for the boot", settings.programGenerationIntent.first())
    }

    @Test
    fun `a confirmed regenerate still discards the open workout`() = runBlocking {
        val workout = openWorkout()

        assertTrue(programRepository.generate(GenerationParams(3), Equipment.entries.toSet(), emptySet(), emptySet()))

        assertNull(db.sessionDao().get(workout))
    }

    @Test
    fun `a block deload held back by an open workout is served once it closes`() = runBlocking {
        cleanDeloadState()
        blockInDeload()
        settings.setBlockDeloadOwed(true)
        val workout = openWorkout()

        repo.serveOwedDeload()
        assertTrue("still owed while the workout is open", settings.blockDeloadOwed.first())
        assertEquals(0L, settings.deloadWeekStartMs.first())

        db.sessionDao().delete(db.sessionDao().get(workout)!!)
        repo.serveOwedDeload()

        assertFalse(settings.blockDeloadOwed.first())
        assertTrue("the deload week was generated", settings.deloadWeekStartMs.first() > 0L)
    }

    @Test
    fun `an owed deload is dropped once the block has left its deload week`() = runBlocking {
        cleanDeloadState()
        db.trainingBlockDao().insert(BlockPlanner.start(nowMs = NOW, weekId = WEEK))
        settings.setBlockDeloadOwed(true)

        repo.serveOwedDeload()

        assertFalse(settings.blockDeloadOwed.first())
        assertEquals(0L, settings.deloadWeekStartMs.first())
    }
}
