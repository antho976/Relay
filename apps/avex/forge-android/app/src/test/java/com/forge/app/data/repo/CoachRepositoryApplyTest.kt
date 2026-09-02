package com.forge.app.data.repo

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.db.entities.CoachPass
import com.forge.app.data.db.entities.OverlaySource
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.coach.TrustLedger
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.Program
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The apply path's two promises, against a real Room database.
 *
 * H-03: the overlay write and the decision-ledger stamp commit as ONE transaction, so a failure
 * between them (the audit's reproduction) leaves nothing behind and a retry applies exactly once.
 * They used to be separate autocommits: the override reached 4 sets, the row stayed proposed, and
 * the retry read 4 and wrote 5 — with undo recording 4 as the "before" state.
 *
 * M-08: a swap or rep shift that performs nothing — its slot is gone, or already at the proposed
 * value — is retired, not recorded as applied, and never counts toward auto-apply trust.
 *
 * The repository is built for real (every dependency is a thin wrapper over the same in-memory DB,
 * the way [com.forge.app.data.importer.WorkoutImportRepositoryTest] builds its repository); none
 * of the heavier collaborators are exercised by these paths.
 */
@RunWith(RobolectricTestRunner::class)
class CoachRepositoryApplyTest {

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val db: ForgeDatabase = inMemoryForgeDb()

    /**
     * The failure injector. In the swap / rep-shift / volume branches the ONLY clock read sits in
     * markAppliedNow — after the overlay write and before the ledger stamp, exactly the gap the
     * audit's reproduction fails in. Flip this and the next apply dies inside that gap.
     */
    private var clockFails = false
    private val clock = Clock {
        if (clockFails) throw IllegalStateException("injected: died between overlay write and ledger stamp")
        NOW
    }

    private val settings = SettingsRepository(context, clock)
    private val customizationRepo = CustomizationRepository(db.exerciseCustomizationDao(), db.dayNameOverrideDao())
    private val programCustomizationRepo = ProgramCustomizationRepository(db.programCustomizationDao())

    // ProgramRepository and AdaptationRepository depend on each other; Hilt breaks the cycle with a
    // dagger.Lazy, and the lateinit + lambda below reproduces that wiring.
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
    private val repo: CoachRepository

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
        val blockRepository = BlockRepository(
            blockDao = db.trainingBlockDao(),
            adaptationRepository = adaptationRepository,
            coachGoalRepository = coachGoalRepository,
            academyRepository = academyRepository,
            settingsRepository = settings,
            database = db,
            clock = clock
        )
        repo = CoachRepository(
            coachDao = db.coachDao(),
            adaptationRepository = adaptationRepository,
            blockRepository = blockRepository,
            settings = settings,
            vacationDao = db.vacationDao(),
            customizationRepo = customizationRepo,
            programCustomizationRepo = programCustomizationRepo,
            programCustomizationDao = db.programCustomizationDao(),
            exerciseCustomizationDao = db.exerciseCustomizationDao(),
            database = db,
            clock = clock
        )
    }

    @After
    fun tearDown() = db.close()

    /** The slot every decision here targets: the seed program's first Upper A exercise. */
    private val slot = Program.days.first { it.key == DAY }.exercises.first()

    /** A same-muscle library exercise that is NOT what the slot already shows — a real swap target. */
    private val replacement = ExerciseLibrary.all.first { it.muscle == slot.muscle && it.name != slot.name }

    /** A rep range that differs from the slot's own — a real rep shift. */
    private val newRange = if (slot.reps == "6-8") "10-12" else "6-8"

    private suspend fun proposed(type: String, payload: String?, targetKey: String = slot.id): Long {
        db.coachDao().insertPass(CoachPass(WEEK, NOW, CoachRepository.STATUS_PROPOSED, null))
        db.coachDao().insertDecisions(
            listOf(
                CoachDecision(
                    weekId = WEEK, type = type, targetKey = targetKey, targetName = slot.name,
                    summary = "s", reason = "r", status = CoachRepository.STATUS_PROPOSED,
                    dayKey = DAY, payload = payload, scopeKey = WEEK
                )
            )
        )
        return db.coachDao().decisionsFor(WEEK).maxOf { it.id }
    }

    private suspend fun decision(id: Long): CoachDecision = db.coachDao().decision(id)!!

    private suspend fun applyExpectingInjectedFailure(id: Long) {
        clockFails = true
        try {
            repo.applyDecision(id)
            fail("the injected failure should have propagated out of applyDecision")
        } catch (expected: IllegalStateException) {
            // The transaction threw — what matters is what it left behind, asserted by the caller.
        } finally {
            clockFails = false
        }
    }

    // ── H-03: one transaction, so a retry applies exactly once ─────────────────

    @Test
    fun volumeUp_failureAfterTheOverlayWrite_rollsBackSoTheRetryAppliesExactlyOnce() = runTest {
        val id = proposed("volume_up", payload = (slot.sets + 1).toString())

        applyExpectingInjectedFailure(id)

        // The audit's reproduction: the override used to survive the failure while the row stayed
        // proposed, so the retry read the mutated count and added another set. Now nothing outlives
        // the failed transaction.
        assertNull(programCustomizationRepo.overrideFor(DAY, slot.id))
        assertEquals(CoachRepository.STATUS_PROPOSED, decision(id).status)

        repo.applyDecision(id)

        val row = programCustomizationRepo.overrideFor(DAY, slot.id)!!
        assertEquals(slot.sets + 1, row.setsOverride)
        assertEquals(OverlaySource.COACH, row.source)
        assertEquals(CoachRepository.STATUS_APPLIED, decision(id).status)
        assertEquals("undo must record the TRUE before-state (no override)", "0", decision(id).undoData)

        // An applied row is inert: a second tap changes nothing.
        repo.applyDecision(id)
        assertEquals(slot.sets + 1, programCustomizationRepo.overrideFor(DAY, slot.id)!!.setsOverride)
    }

    @Test
    fun swap_failureAfterTheOverlayWrite_leavesNoSwapBehind() = runTest {
        val id = proposed("swap", payload = replacement.id)

        applyExpectingInjectedFailure(id)

        assertNull(customizationRepo.getSwap(slot.id))
        assertEquals(CoachRepository.STATUS_PROPOSED, decision(id).status)

        repo.applyDecision(id)

        val swap = customizationRepo.getSwap(slot.id)!!
        assertEquals(replacement.name, swap.swappedName)
        assertEquals(replacement.id, swap.swappedExerciseId)
        assertEquals(OverlaySource.COACH, swap.source)
        assertEquals(CoachRepository.STATUS_APPLIED, decision(id).status)
        assertEquals("no prior swap → undo removes, not restores", "∅", decision(id).undoData)
    }

    @Test
    fun repShift_failureAfterTheOverlayWrite_leavesNoOverrideBehind() = runTest {
        val id = proposed("rep_shift", payload = newRange)

        applyExpectingInjectedFailure(id)

        assertNull(programCustomizationRepo.overrideFor(DAY, slot.id))
        assertEquals(CoachRepository.STATUS_PROPOSED, decision(id).status)

        repo.applyDecision(id)

        assertEquals(newRange, programCustomizationRepo.overrideFor(DAY, slot.id)!!.repRangeOverride)
        assertEquals(CoachRepository.STATUS_APPLIED, decision(id).status)
        assertEquals("∅", decision(id).undoData)
    }

    // ── M-08: an unperformed change is not an applied change ───────────────────

    @Test
    fun aSwapWhoseSlotIsNotInTheProgram_isRetiredNotApplied() = runTest {
        val id = proposed("swap", payload = replacement.id, targetKey = "no-such-slot")

        repo.applyDecision(id)

        val d = decision(id)
        assertEquals(CoachRepository.STATUS_SKIPPED, d.status)
        assertEquals(CoachDecision.OUTCOME_NOT_FOLLOWED, d.outcome)
        assertNull(d.appliedAt)
        assertNull(customizationRepo.getSwap("no-such-slot"))
    }

    @Test
    fun aSwapToTheExerciseAlreadyInTheSlot_isRetiredNotApplied() = runTest {
        // A genuine rotation first...
        val first = proposed("swap", payload = replacement.id)
        repo.applyDecision(first)
        assertEquals(CoachRepository.STATUS_APPLIED, decision(first).status)

        // ...then the identical proposal again: the slot already shows it, so nothing is performed.
        val again = proposed("swap", payload = replacement.id)
        repo.applyDecision(again)

        assertEquals(CoachRepository.STATUS_SKIPPED, decision(again).status)
        assertEquals(CoachDecision.OUTCOME_NOT_FOLLOWED, decision(again).outcome)
        assertEquals("the real change is untouched", CoachRepository.STATUS_APPLIED, decision(first).status)
        assertEquals(replacement.id, customizationRepo.getSwap(slot.id)!!.swappedExerciseId)
    }

    @Test
    fun aRepShiftToTheRangeTheSlotAlreadyHas_isRetiredNotApplied() = runTest {
        val id = proposed("rep_shift", payload = slot.reps)

        repo.applyDecision(id)

        assertEquals(CoachRepository.STATUS_SKIPPED, decision(id).status)
        assertEquals(CoachDecision.OUTCOME_NOT_FOLLOWED, decision(id).outcome)
        assertNull(programCustomizationRepo.overrideFor(DAY, slot.id))
    }

    @Test
    fun unperformedChanges_neverEarnAutoApplyTrust() = runTest {
        // Exactly the conservative streak's worth of rep shifts — none of which performed anything.
        repeat(TrustLedger.CONSERVATIVE_STREAK) {
            repo.applyDecision(proposed("rep_shift", payload = slot.reps))
        }
        val all = db.coachDao().allDecisions()
        assertEquals(TrustLedger.CONSERVATIVE_STREAK, all.size)
        assertTrue(all.all { it.status == CoachRepository.STATUS_SKIPPED && it.outcome == CoachDecision.OUTCOME_NOT_FOLLOWED })
        assertEquals(0, TrustLedger.assess(all).first { it.type == "rep_shift" }.streak)
        assertTrue(TrustLedger.earnedTypes(all).isEmpty())

        // Nor do they break a streak: a real change afterwards starts counting from one.
        repo.applyDecision(proposed("rep_shift", payload = newRange))
        assertEquals(1, TrustLedger.assess(db.coachDao().allDecisions()).first { it.type == "rep_shift" }.streak)
    }

    private companion object {
        const val NOW = 1_700_000_000_000L
        const val WEEK = "2026-W10"
        const val DAY = Program.UPPER_A
    }
}
