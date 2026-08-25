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
import com.forge.app.domain.units.toStoredWeightText

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
 * - Hard jump bound: a set >1.5× the all-time frontier max (or a 3+ plate jump) answers
 *   needsConfirm — one wrist confirm tap resends it [LogSetCommand.confirmedJump] (the wrist's
 *   dialog-free equivalent of the phone's jump-confirm). A misspun bezel still can't poison PRs
 *   silently: nothing big lands without the second tap.
 * - Undo is repository-level and time-bounded: the last set of the active session, within
 *   [UNDO_WINDOW_MS], deletable with no ViewModel alive.
 * - RPE targets the exact [CmdAckDto.setId] the log ack named, within [RPE_WINDOW_MS] — works
 *   after the session finishes (rating the last set), and an undone set just answers "set gone".
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
    private val focusHolder: com.forge.app.service.wear.WearFocusHolder,
    private val clock: Clock
) {
    data class Result(
        val ok: Boolean,
        val reason: String? = null,
        val wasPr: Boolean = false,
        /** The log needs a wrist confirm tap (big jump) — not an error, not logged yet. */
        val needsConfirm: Boolean = false,
        /** The written set's row id on success — rides the ack so wrist RPE can target it. */
        val setId: Long? = null
    )

    suspend fun logFromWatch(cmd: LogSetCommand): Result {
        val session = sessionDao.getActiveSession() ?: return Result(false, "no active session")
        if (session.id != cmd.sessionId) return Result(false, "session changed")
        // Freestyle sessions never mirror to the wrist (WatchSessionMirror gates them), so no
        // watch command should carry their id — but Program.day("freestyle") falls back to the
        // program's first day, so a stale command must be refused, never logged against that day.
        if (session.dayKey == Program.FREESTYLE_DAY_KEY) {
            return Result(false, "freestyle logs live on the phone")
        }

        // ── Resolve the current slot exactly as the wrist saw it (mirror logic) ──
        val plan = Program.day(session.dayKey)
        if (plan.exercises.isEmpty()) return Result(false, "no plan")
        val logged = loggedExerciseDao.forSession(session.id)
        val allSets = loggedSetDao.allForSession(session.id)
        val setsByLogged = allSets.groupBy { it.loggedExerciseId }
        fun loggedFor(planId: String) =
            logged.firstOrNull { it.slotId == planId || (it.slotId == null && it.exerciseId == planId) }

        // The SAME current-slot policy the mirror displays (CurrentSlotResolver) — what the wrist
        // showed is what this log targets.
        val rows = plan.exercises.map { ex -> ex to loggedFor(ex.id) }
        val lastLoggedIdx = allSets.maxByOrNull { it.completedAt }
            ?.let { last -> rows.indexOfFirst { (_, row) -> row?.id == last.loggedExerciseId } }
            ?.takeIf { it >= 0 }
        val earlyDoneIdx = focusHolder.earlyDoneFor(session.id)
            .let { ids -> rows.indices.filter { rows[it].first.id in ids }.toSet() }
        val currentIdx = com.forge.app.service.wear.CurrentSlotResolver.resolve(
            plannedSets = rows.map { it.first.sets },
            doneSets = rows.map { (_, row) -> row?.let { setsByLogged[it.id]?.size } ?: 0 },
            lastLoggedIdx = lastLoggedIdx,
            earlyDoneIdx = earlyDoneIdx
        )
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
        // cmd.weightText comes from the WRIST, in the user's DISPLAY unit — that is what the watch
        // renders and steps. Every other source below is a STORED weightText, which is already lb.
        // Mixing the two is how a kg user's wrist entry of "60 KG" was stored as 60 lb: convert the
        // wrist value to lb here, and leave the stored fallbacks alone.
        //
        // PLATES is exempt: its text is a plate count, which carries no unit, and running it through
        // a kg conversion would be the same bug in reverse.
        val weightUnit = settingsRepo.weightUnit.first()
        val fromWrist = cmd.weightText?.let {
            if (effectivePlan.unit == ExerciseUnit.PLATES) it else toStoredWeightText(it, weightUnit)
        }
        val weightText = fromWrist
            ?: sessionPrefill(row?.id, setsByLogged)
            ?: workoutRepo.lastPerformanceSets(effectiveId).lastOrNull()?.weightText
            ?: if (isBodyweight) "BW" else return Result(false, "no target yet, pick a weight")
        val reps = cmd.reps ?: prescribedReps(effectivePlan)
            ?: return Result(false, "no rep target, pick reps")
        if (reps <= 0) return Result(false, "reps must be at least 1")
        val weightLb = WeightParser.parse(weightText, effectivePlan.unit, plateLb)

        // ── Hard jump bound (wrist policy — a confirm TAP, no dialogs on a 1.4" screen) ──
        if (!cmd.confirmedJump) {
            val frontierMax = workoutRepo.repMaxFrontierForExercise(effectiveId, null)
                .mapNotNull { it.weightLb }.maxOrNull()
            if (weightLb != null && frontierMax != null && frontierMax > 0) {
                val isPlates = effectivePlan.unit == ExerciseUnit.PLATES
                val bigJump =
                    if (isPlates) (weightLb - frontierMax) / plateLb >= 3.0
                    else weightLb > frontierMax * 1.5
                if (bigJump) return Result(false, reason = "big jump", needsConfirm = true)
            }
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
        val setId = workoutRepo.logSet(
            loggedExerciseId = leId,
            setIndex = doneCount,
            weightText = weightText,
            weightLb = weightLb,
            reps = reps
        )
        // PR at write time — the repository-level pass freestyle logging already uses, so the wrist
        // gold moment and the phone's flags agree.
        val wasPr = workoutRepo.flagPrForLoggedExercise(leId, effectiveId)
        return Result(true, wasPr = wasPr, setId = setId)
    }

    /** Rate the set a log ack named — targeted by row id, so it can't land on the wrong set. */
    suspend fun rpeFromWatch(setId: Long, rpe: Double): Result {
        if (rpe < 6.0 || rpe > 10.0) return Result(false, "rpe out of range")
        val set = loggedSetDao.get(setId) ?: return Result(false, "set gone")
        if (clock.nowMs() - set.completedAt > RPE_WINDOW_MS) {
            return Result(false, "too late to rate here, use the phone")
        }
        workoutRepo.setRpe(setId, rpe)
        return Result(true, setId = setId)
    }

    /**
     * Undo the set the wrist named — repository-level, bounded to a short window.
     *
     * [setId] comes off that set's own log ack, so this deletes the row the user was looking at.
     * Resolving "the session's most recent set" instead meant a set logged on the phone between the
     * wrist's set and the undo tap was the one that vanished, and a double-tapped undo deleted two
     * different sets. A null [setId] is an older wrist build and keeps the previous behaviour; a
     * setId that is no longer there (already undone — the second of two taps) is not an error worth
     * a second deletion.
     */
    suspend fun undoLastFromWatch(sessionId: Long, setId: Long? = null): Result {
        val session = sessionDao.getActiveSession() ?: return Result(false, "no active session")
        if (session.id != sessionId) return Result(false, "session changed")
        val sets = loggedSetDao.allForSession(session.id)
        val target = if (setId != null) {
            sets.firstOrNull { it.id == setId } ?: return Result(false, "already undone")
        } else {
            sets.maxByOrNull { it.completedAt } ?: return Result(false, "nothing to undo")
        }
        if (clock.nowMs() - target.completedAt > UNDO_WINDOW_MS) {
            return Result(false, "too late to undo here, use the phone")
        }
        workoutRepo.deleteSet(target)
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

        /** How long after a set the wrist may still rate it. Generous — the natural moment is
         *  during the rest that follows, and the id-targeted write can't hit the wrong set. */
        const val RPE_WINDOW_MS = 10 * 60_000L
    }
}
