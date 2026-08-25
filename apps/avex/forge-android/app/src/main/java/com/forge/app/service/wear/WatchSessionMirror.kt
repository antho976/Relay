package com.forge.app.service.wear

import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.pr.PrDetector
import com.forge.app.domain.units.WeightUnit
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.Program
import com.forge.shared.protocol.SessionLiveDto
import com.forge.shared.weight.ProtocolWeightUnit
import com.forge.shared.weight.WeightSteps
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import javax.inject.Inject
import javax.inject.Singleton
import com.forge.app.domain.units.weightInputValue

/** :app's WeightUnit as it travels over the wear protocol. */
fun WeightUnit.toProtocol(): ProtocolWeightUnit = when (this) {
    WeightUnit.KG -> ProtocolWeightUnit.KG
    WeightUnit.ST -> ProtocolWeightUnit.ST
    else -> ProtocolWeightUnit.LB
}

/**
 * Builds the /session/live mirror REPOSITORY-side (W1) — from Room flows + the Program facade,
 * never from a ViewModel — so the wrist stays correct when no day screen exists and after the
 * phone process is killed and rewoken by a watch command. Every set log/undo re-emits through
 * Room invalidation.
 *
 * W1 scope: identity, current exercise, set counts, elapsed anchor, bezel metadata. The
 * prescribed-target text and PR flag land with W2's SetLogUseCase, which owns that resolution.
 */
@Singleton
class WatchSessionMirror @Inject constructor(
    sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao,
    private val customizationRepo: com.forge.app.data.repo.CustomizationRepository,
    private val settingsRepo: SettingsRepository,
    private val focusHolder: WearFocusHolder
) {
    @OptIn(ExperimentalCoroutinesApi::class)
    val sessionLive: Flow<SessionLiveDto?> = sessionDao.observeActiveSession()
        .flatMapLatest { session ->
            // Freestyle ("Open workout") logs are draft-built on the phone and exist as an active
            // session only for the instant save() runs — never a live sitting the wrist could join.
            // Without this gate Program.day("freestyle") falls back to the program's FIRST day and
            // the watch would flash (or log against) a day the user isn't doing. Wrist freestyle
            // is a deliberate later feature; until then the watch stays idle.
            if (session == null || session.dayKey == Program.FREESTYLE_DAY_KEY) flowOf(null)
            else combine(
                loggedExerciseDao.observeForSession(session.id),
                loggedSetDao.observeAllForSession(session.id),
                settingsRepo.weightUnit,
                focusHolder.earlyDone
            ) { exercises, sets, unit, _ -> buildDto(session, exercises, sets, unit) }
        }

    private suspend fun buildDto(
        session: Session,
        logged: List<LoggedExercise>,
        sets: List<LoggedSet>,
        unit: WeightUnit
    ): SessionLiveDto {
        val plan = Program.day(session.dayKey)
        val setsByLoggedExercise = sets.groupBy { it.loggedExerciseId }
        // Per-plan-slot logged rows: a slot's row matches by slotId (swapped) or exerciseId (direct).
        fun loggedFor(planId: String): LoggedExercise? =
            logged.firstOrNull { it.slotId == planId || (it.slotId == null && it.exerciseId == planId) }

        val counts = plan.exercises.map { ex ->
            val row = loggedFor(ex.id)
            val done = row?.let { setsByLoggedExercise[it.id]?.size } ?: 0
            Triple(ex, row, done)
        }
        // Current slot = CurrentSlotResolver's policy (follow the lifter): the most recently
        // logged slot while short of plan, else the next incomplete AFTER it, skipping slots the
        // day screen declared done early (WearFocusHolder) — never pinned to an abandoned slot.
        val lastLoggedIdx = sets.maxByOrNull { it.completedAt }
            ?.let { last -> counts.indexOfFirst { (_, row, _) -> row?.id == last.loggedExerciseId } }
            ?.takeIf { it >= 0 }
        val earlyDoneIdx = focusHolder.earlyDoneFor(session.id)
            .let { ids -> counts.indices.filter { counts[it].first.id in ids }.toSet() }
        val currentIdx = CurrentSlotResolver.resolve(
            plannedSets = counts.map { it.first.sets },
            doneSets = counts.map { it.third },
            lastLoggedIdx = lastLoggedIdx,
            earlyDoneIdx = earlyDoneIdx
        )
        val current = counts.getOrNull(currentIdx)
        val protoUnit = unit.toProtocol()

        // Effective exercise = the swapped one, even before its first set (#11) — same resolution
        // as SetLogUseCase, so what the wrist shows is exactly what a log would write.
        val swap = current?.let { customizationRepo.getSwap(it.first.id) }
        val effectiveId = current?.second?.exerciseId
            ?: swap?.swappedExerciseId?.takeIf { it.isNotBlank() }
            ?: current?.first?.id
        val effectivePlan = effectiveId?.let { Program.exercise(it) } ?: current?.first
        val isPlates = effectivePlan?.unit == ExerciseUnit.PLATES

        // The wrist's target weight = the same prefill the phone's input seeds from: the last set
        // of this exercise IN this session, else the last performance in history. (The coach's
        // suggestion chip stays a phone surface; W2 echoes whatever the wrist DISPLAYED.)
        val targetWeightText = current?.let { (_, row, _) ->
            row?.let { setsByLoggedExercise[it.id]?.maxByOrNull { s -> s.setIndex }?.weightText }
                ?: effectiveId?.let { id -> lastPerformanceWeightText(id) }
        }?.let { stored -> toWristWeightText(stored, unit, isPlates) }
        // The most recent set in the session just went PR — the wrist's gold moment (also carried
        // on the log ack; this covers phone-logged PRs while the wrist mirrors).
        //
        // `wasPr` lives on the EXERCISE, so reading it alone made this a per-exercise flag: once any
        // set of a lift set a record, every later SessionLiveDto for that lift kept reporting the
        // last set as a PR, including the back-off sets after it. The exercise flag is still the
        // gate — it is the only thing that knows about all-time history — but the last set must also
        // be the one that earned it: a set that beats every earlier set of this exercise in this
        // session at its own rep count, which is what PrDetector tests.
        val lastSetWasPr = sets.maxByOrNull { it.completedAt }?.let { lastSet ->
            val exercise = logged.firstOrNull { it.id == lastSet.loggedExerciseId }
            exercise?.wasPr == true && !lastSet.isAssisted && PrDetector.isPr(
                history = setsByLoggedExercise[lastSet.loggedExerciseId]
                    ?.filter { it.id != lastSet.id && it.completedAt <= lastSet.completedAt }
                    .orEmpty(),
                newWeightLb = lastSet.weightLb,
                newReps = lastSet.reps
            )
        } ?: false

        return SessionLiveDto(
            sessionId = session.id,
            dayTitle = Program.dayDisplayName(session.dayKey),
            exerciseId = current?.first?.id,
            exerciseName = current?.let { (ex, row, _) ->
                row?.swappedName?.takeIf { it.isNotBlank() }
                    ?: swap?.swappedName?.takeIf { it.isNotBlank() }
                    ?: effectivePlan?.name ?: ex.name
            },
            exerciseIndex = if (current == null) 0 else currentIdx + 1,
            exerciseCount = plan.exercises.size,
            setIndex = current?.let { (ex, _, done) -> (done + 1).coerceAtMost(ex.sets) } ?: 0,
            setTotal = current?.first?.sets ?: 0,
            targetWeightText = targetWeightText,
            targetRepsText = current?.first?.reps,
            loggedSets = current?.third ?: 0,
            lastSetWasPr = lastSetWasPr,
            startedAtMs = session.startedAt,
            unit = protoUnit,
            weightStep = WeightSteps.weightStep(protoUnit, isPlates),
            isPlates = isPlates,
            isBodyweight = effectivePlan?.unit == ExerciseUnit.BODYWEIGHT
        )
    }

    /**
     * Stored weight text is POUNDS (see [com.forge.app.data.db.entities.LoggedSet]), but the DTO's
     * `unit` and `weightStep` below are the user's DISPLAY unit. Sending the stored text unconverted
     * put all three out of agreement: the wrist rendered "220.5" under a "KG" label for a 100 kg
     * lift, and each 2.5 detent added 2.5 kg worth of step to a pound number.
     *
     * PLATES is exempt — its text is a plate COUNT and `weightStep` is already a plate step, so it
     * carries no unit to convert. Non-numeric notation ("BW", "2 plates") passes through unchanged.
     */
    private fun toWristWeightText(stored: String, unit: WeightUnit, isPlates: Boolean): String {
        if (isPlates) return stored
        val lb = stored.trim().toDoubleOrNull() ?: return stored
        return weightInputValue(lb, unit)
    }

    /** The weight text of the most recent historical performance of [exerciseId] (prefill rule). */
    private suspend fun lastPerformanceWeightText(exerciseId: String): String? {
        val last = loggedExerciseDao.lastLoggedBefore(exerciseId, -1L) ?: return null
        return loggedSetDao.forLoggedExercise(last.id).maxByOrNull { it.setIndex }?.weightText
    }
}
