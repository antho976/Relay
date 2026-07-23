package com.forge.app.domain.session

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.CustomizationRepository
import com.forge.app.data.repo.WorkoutRepository
import com.forge.app.domain.adapt.RestAdvisor
import com.forge.app.domain.parser.WeightParser
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.Program
import com.forge.app.service.wear.SessionTimerHolder
import com.forge.shared.protocol.LogSetCommand
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The watch's entry into the ONE set-write path (W2). Resolves session context from Room + the
 * Program facade with NO ViewModel — a killed-then-rewoken process (WearableListenerService wakes
 * it; ForgeApp re-seeds Program) can process a wrist log with nothing but this class. The write
 * itself is exactly the phone UI's: [WorkoutRepository.addExerciseToSession] (lazy) +
 * [WorkoutRepository.logSet] + the repository-level PR pass ([WorkoutRepository.flagPrForLoggedExercise],
 * the same one freestyle logging uses). Phone-only UI side effects (jump-confirm dialog,
 * auto-collapse, the day screen's 5s undo chip) stay in the day handlers.
 *
 * Wrist-specific policy:
 * - The watch always echoes back the weight/reps it DISPLAYED (seeded from /session/live's target);
 *   a command with no weight falls back to the same prefill the phone input uses (last performance).
 * - Hard jump bound: a set >1.5× the all-time frontier max (or a 3+ plate jump) is refused with a
 *   reason — a misspun bezel must not poison PRs and calibration. The phone's softer 1.2× rule
 *   stays a phone-side CONFIRM; the wrist gets a hard cap instead of a dialog.
 * - Undo is repository-level and time-bounded: the last set of the active session, within
 *   [UNDO_WINDOW_MS], deletable with no ViewModel alive.
 */
@Singleton
class SetLogUseCase @Inject constructor(
    private val workoutRepo: WorkoutRepository,
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao,
    private val customizationRepo: CustomizationRepository,
    private val settingsRepo: SettingsRepository,
    private val timerHolder: SessionTimerHolder,
    private val clock: Clock
) {
    data class Result(val ok: Boolean, val reason: String? = null, val wasPr: Boolean = false)

    suspend fun logFromWatch(cmd: LogSetCommand): Result {
        val session = sessionDao.getActiveSession() ?: return Result(false, "no active session")
        if (session.id != cmd.sessionId) return Result(false, "session changed")

        // ── Resolve the current slot exactly as the wrist saw it (mirror logic) ──
        val plan = Program.day(session.dayKey)
        if (plan.exercises.isEmpty()) return Result(false, "no plan")
        val logged = loggedExerciseDao.forSession(session.id)
        val setsByLogged = loggedSetDao.allForSession(session.id).groupBy { it.loggedExerciseId }
        fun loggedFor(planId: String) =
            logged.firstOrNull { it.slotId == planId || (it.slotId == null && it.exerciseId == planId) }

        val currentIdx = plan.exercises.indexOfFirst { ex ->
            val row = loggedFor(ex.id)
            (row?.let { setsByLogged[it.id]?.size } ?: 0) < ex.sets
        }.let { if (it == -1) plan.exercises.lastIndex else it }
        val slotPlan = plan.exercises[currentIdx]
        if (cmd.exerciseId != null && cmd.exerciseId != slotPlan.id) {
            // The wrist showed a different exercise than is now current (a phone log advanced it
            // mid-flight) — refuse rather than logging against the wrong slot.
            return Result(false, "exercise moved on")
        }
        val row = loggedFor(slotPlan.id)
        val doneCount = row?.let { setsByLogged[it.id]?.size } ?: 0

        // Persistent swap (#11): the slot logs under the swapped exercise even before its first set.
        val swap = customizationRepo.getSwap(slotPlan.id)
        val effectiveId = row?.exerciseId
            ?: swap?.swappedExerciseId?.takeIf { it.isNotBlank() }
            ?: slotPlan.id
        val effectivePlan = Program.exercise(effectiveId) ?: slotPlan

        // ── Resolve weight + reps (echoed from the wrist, prefilled when absent) ──
        val plateLb = settingsRepo.plateWeightLb.first()
        val isBodyweight = effectivePlan.unit == ExerciseUnit.BODYWEIGHT
        val weightText = cmd.weightText
            ?: sessionPrefill(row?.id, setsByLogged)
            ?: workoutRepo.lastPerformanceSets(effectiveId).lastOrNull()?.weightText
            ?: if (isBodyweight) "BW" else return Result(false, "no target yet — pick a weight")
        val reps = cmd.reps ?: prescribedReps(effectivePlan)
            ?: return Result(false, "no rep target — pick reps")
        if (reps <= 0) return Result(false, "reps must be at least 1")
        val weightLb = WeightParser.parse(weightText, effectivePlan.unit, plateLb)

        // ── Hard jump bound (wrist policy — no dialogs on a 1.4" screen) ──
        val frontierMax = workoutRepo.repMaxFrontierForExercise(effectiveId, null)
            .mapNotNull { it.weightLb }.maxOrNull()
        if (weightLb != null && frontierMax != null && frontierMax > 0) {
            val isPlates = effectivePlan.unit == ExerciseUnit.PLATES
            val bigJump =
                if (isPlates) (weightLb - frontierMax) / plateLb >= 3.0
                else weightLb > frontierMax * 1.5
            if (bigJump) return Result(false, "big jump — log it on the phone")
        }

        // ── Rest timer first (the wrist should feel the countdown instantly), then the write ──
        val restOverride = swap?.restTimerOverrideSeconds
        val rest = RestAdvisor.restSeconds(
            plan = effectivePlan,
            lastEffort = null,
            overrideSeconds = restOverride,
            compoundBase = settingsRepo.restCompoundSeconds.first(),
            isolationBase = settingsRepo.restIsolationSeconds.first()
        )
        timerHolder.controller.start(rest.seconds)

        val leId = row?.id ?: workoutRepo.addExerciseToSession(
            sessionId = session.id,
            exerciseId = effectiveId,
            orderIndex = currentIdx,
            swappedName = swap?.swappedName?.takeIf { it.isNotBlank() },
            swappedUnit = swap?.swappedUnit?.takeIf { it.isNotBlank() },
            slotId = slotPlan.id.takeIf { it != effectiveId }
        )
        workoutRepo.logSet(
            loggedExerciseId = leId,
            setIndex = doneCount,
            weightText = weightText,
            weightLb = weightLb,
            reps = reps
        )
        // PR at write time — the repository-level pass freestyle logging already uses, so the wrist
        // gold moment and the phone's flags agree.
        val wasPr = workoutRepo.flagPrForLoggedExercise(leId, effectiveId)
        return Result(true, wasPr = wasPr)
    }

    /** Undo the last set of the active session — repository-level, bounded to a short window. */
    suspend fun undoLastFromWatch(sessionId: Long): Result {
        val session = sessionDao.getActiveSession() ?: return Result(false, "no active session")
        if (session.id != sessionId) return Result(false, "session changed")
        val last = loggedSetDao.allForSession(session.id).maxByOrNull { it.completedAt }
            ?: return Result(false, "nothing to undo")
        if (clock.nowMs() - last.completedAt > UNDO_WINDOW_MS) {
            return Result(false, "too late to undo here — use the phone")
        }
        workoutRepo.deleteSet(last)
        return Result(true)
    }

    /** The weight of the last set already logged for this exercise IN THIS SESSION, if any. */
    private fun sessionPrefill(loggedExerciseId: Long?, setsByLogged: Map<Long, List<com.forge.app.data.db.entities.LoggedSet>>): String? =
        loggedExerciseId?.let { id -> setsByLogged[id]?.maxByOrNull { it.setIndex }?.weightText }

    /** As-prescribed reps = the range's lower bound ("8-12" → 8, "10" → 10); null if unparsable. */
    private fun prescribedReps(plan: com.forge.app.program.ExercisePlan): Int? =
        plan.reps.takeWhile { it.isDigit() }.toIntOrNull()

    companion object {
        /** How long after a set the wrist may still undo it. Wider than the phone chip's 5 s — the
         *  wrist round-trip (tap → mirror update → notice → undo) needs the slack. */
        const val UNDO_WINDOW_MS = 15_000L
    }
}
