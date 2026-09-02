package com.forge.app.data.repo

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.coach.BlockPlanner
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * M-13: the training block is a singleton, and the repository is what keeps it one.
 *
 * Two rapid Start taps used to run two `active()` reads before either insert, and both inserted.
 * `ORDER BY started_at DESC LIMIT 1` then hid one of them; ending the visible block ended only that
 * row, and the hidden one came back as the live block. Like [com.forge.app.data.db.SessionWritesConcurrencyTest]
 * these run genuinely parallel callers under `runBlocking` rather than a single-scheduler `runTest`,
 * because the sentence they are about is "both taps ran".
 *
 * The repository is built for real over one in-memory database, the way [CoachRepositoryApplyTest]
 * builds its collaborators.
 */
@RunWith(RobolectricTestRunner::class)
class BlockRepositoryConcurrencyTest {

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
            exerciseGoalDao = db.exerciseGoalDao(),
            extendedGoalDao = db.extendedGoalDao(),
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

        /** Enough parallelism to lose reliably without the lock; small enough to stay fast. */
        const val RACERS = 12
    }

    @Test
    fun `concurrent starts agree on one block`() = runBlocking {
        val blocks = withContext(Dispatchers.Default) {
            (1..RACERS).map { async { repo.start(weekId = WEEK) } }.awaitAll()
        }

        assertEquals("every caller must be handed the SAME block", 1, blocks.map { it.id }.toSet().size)
        assertEquals("and exactly one open row exists", 1, db.trainingBlockDao().allActive().size)
        assertEquals("with nothing inserted and then hidden", 1, db.trainingBlockDao().all().size)
    }

    @Test
    fun `a second start after the first resumes it`() = runBlocking {
        val first = repo.start(weekId = WEEK)
        val second = repo.start(weekId = WEEK)

        assertEquals(first.id, second.id)
        assertEquals(1, db.trainingBlockDao().all().size)
    }

    @Test
    fun `an extra open row left by an older build is ended on read`() = runBlocking {
        // Two open rows, as the unguarded start used to leave behind.
        db.trainingBlockDao().insert(BlockPlanner.start(nowMs = NOW, weekId = WEEK))
        val newest = db.trainingBlockDao().insert(BlockPlanner.start(nowMs = NOW + 1, weekId = WEEK))

        val visible = repo.active()

        assertEquals("the row the user could already see is the one kept", newest, visible?.id)
        assertEquals("the hidden duplicate is ended by the read", 1, db.trainingBlockDao().allActive().size)
        assertEquals("and kept in history rather than deleted", 2, db.trainingBlockDao().all().size)
    }

    @Test
    fun `ending the block leaves no open row behind`() = runBlocking {
        db.trainingBlockDao().insert(BlockPlanner.start(nowMs = NOW, weekId = WEEK))
        db.trainingBlockDao().insert(BlockPlanner.start(nowMs = NOW + 1, weekId = WEEK))

        repo.end()

        assertTrue("no hidden block can outlive the user's veto", db.trainingBlockDao().allActive().isEmpty())
        assertEquals(null, repo.active())
        assertEquals(2, db.trainingBlockDao().all().size)
    }
}
