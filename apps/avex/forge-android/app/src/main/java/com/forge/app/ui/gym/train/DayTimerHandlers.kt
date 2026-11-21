package com.forge.app.ui.gym.train

import com.forge.app.ui.gym.train.state.DayUiEvent
import kotlinx.coroutines.flow.update

internal fun DayViewModel.handleTimerEvent(event: DayUiEvent) {
    when (event) {
        is DayUiEvent.RestTimerOpen -> _state.update { it.copy(showTimerControls = true) }
        is DayUiEvent.RestTimerClose -> _state.update { it.copy(showTimerControls = false) }
        is DayUiEvent.RestTimerPause -> restTimer.pause()
        is DayUiEvent.RestTimerResume -> restTimer.resume()
        is DayUiEvent.RestTimerReset -> restTimer.reset()
        is DayUiEvent.RestTimerSkip -> {
            // Skipping is the strongest "this rest is too long" signal — keep the interval
            // open (rest truly ends at the next set) but remember it was cut deliberately.
            openRestEvent = openRestEvent?.copy(skipped = true)
            restTimer.stop()
            _state.update { it.copy(showTimerControls = false) }
        }
        is DayUiEvent.RestTimerAddSeconds -> {
            openRestEvent = openRestEvent?.let { it.copy(secondsAdded = it.secondsAdded + event.seconds) }
            restTimer.addSeconds(event.seconds)
        }
        is DayUiEvent.RestTimerSetTo -> {
            // An explicit length, counted from now. The presets used to ADD ("+2 min", "+5 min"),
            // so a lifter who tapped "+5 min" meaning "rest 5 minutes" two minutes in got seven.
            // Measured from the set as before: planned = what they have now asked for, in total.
            openRestEvent = openRestEvent?.let { open ->
                val elapsed = ((clock.nowMs() - open.startedAtMs) / 1000L).toInt().coerceAtLeast(0)
                open.copy(plannedSeconds = elapsed + event.seconds, secondsAdded = 0, manual = true)
            }
            restTimer.start(event.seconds)
        }
        else -> {}
    }
}

// ─── Realized-rest capture (adaptation engine System 2, #82) ─────────────────────

/**
 * The rest interval currently being measured. Opened when a set is logged (the timer
 * starts), closed by the NEXT logged set; an interval left open when the session ends
 * is dropped — it wasn't rest that led to another set.
 */
internal data class OpenRestEvent(
    val exerciseId: String,
    val setIndex: Int,
    val plannedSeconds: Int,
    val startedAtMs: Long,
    val secondsAdded: Int = 0,
    val skipped: Boolean = false,
    /** The rest was capped for a light / feeler set — not evidence of the user's pace, never persisted. */
    val light: Boolean = false,
    /** The user picked this rest's length by hand; a later effort rating must not re-price it. */
    val manual: Boolean = false
)

/** Close (and persist) the open rest interval — the set logged at [endedAtMs] ended it. */
internal suspend fun DayViewModel.closeOpenRestEvent(sessionId: Long, endedAtMs: Long) {
    val open = openRestEvent ?: return
    openRestEvent = null
    // A light set's short rest would teach the tuner that this user rests briefly on compounds.
    if (open.light) return
    val realized = ((endedAtMs - open.startedAtMs) / 1000L).toInt()
    if (realized <= 0) return
    val endedBy = when {
        open.skipped -> "skipped"
        realized > open.plannedSeconds + open.secondsAdded -> "expired"
        else -> "next_set"
    }
    workoutRepo.logRestEvent(
        sessionId = sessionId,
        exerciseId = open.exerciseId,
        setIndex = open.setIndex,
        plannedSeconds = open.plannedSeconds,
        realizedSeconds = realized,
        endedBy = endedBy,
        secondsAdded = open.secondsAdded
    )
}

/**
 * A set that is about to be undone or deleted may be the one whose logging started the running rest
 * timer. That rest never happened, so stop the timer and drop the open interval: left in place, the
 * corrected re-log would close it and persist the time spent fixing the entry as a realized rest.
 * Deleting an earlier set leaves the current rest alone.
 */
internal fun DayViewModel.cancelRestForRemovedSet(setId: Long) {
    val open = openRestEvent ?: return
    val ex = _state.value.exercises.firstOrNull { e -> e.loggedSets.any { it.id == setId } } ?: return
    val index = ex.loggedSets.indexOfFirst { it.id == setId }
    if (open.exerciseId != ex.effectiveExerciseId.ifBlank { ex.plan.id } || open.setIndex != index) return
    openRestEvent = null
    restTimer.stop()
    _state.update { it.copy(showTimerControls = false) }
}

/**
 * A removed set that recorded a suggestion outcome (the first set of an exercise, logged while a
 * suggestion chip showed) takes that sample with it. Left in place, the mis-tap stays in the coach's
 * step calibration, and the corrected re-log (loggedSets is empty again) records a second sample.
 */
internal suspend fun DayViewModel.dropSuggestionOutcomeForRemovedSet(setId: Long) {
    val outcomeId = suggestionOutcomeBySetId.remove(setId) ?: return
    workoutRepo.deleteSuggestionOutcome(outcomeId)
}

/** Reprice only the current set's open rest, retaining elapsed time and manual timer adjustments. */
internal fun DayViewModel.updateRestForLatestEffort(exerciseId: String, setId: Long? = null) {
    val open = openRestEvent ?: return
    val timer = restTimer.state.value ?: return
    if (timer.isFinished || open.skipped || open.manual) return
    val ui = _state.value.exercises.firstOrNull { it.plan.id == exerciseId } ?: return
    val latest = ui.loggedSets.lastOrNull() ?: return
    if (setId != null && latest.id != setId) return
    val effectiveId = ui.effectiveExerciseId.ifBlank { exerciseId }
    if (open.exerciseId != effectiveId || open.setIndex != ui.loggedSets.lastIndex) return
    val plan = (com.forge.app.program.Program.exercise(effectiveId) ?: ui.plan).copy(reps = ui.plan.reps)
    val effort = com.forge.app.domain.adapt.RestAdvisor.effortForSet(latest.rpe, latest.difficultyTag, ui.difficulty)
    // Same performed-set view the timer was started with, so re-pricing for a rating can't
    // re-award a heavy bonus or un-cap a light set.
    val performed = com.forge.app.domain.adapt.PerformedSet(
        reps = latest.reps.takeIf { it > 0 },
        weightLb = latest.weightLb,
        referenceWeightLb = listOfNotNull(
            ui.priorFrontier.mapNotNull { it.weightLb }.maxOrNull(),
            ui.loggedSets.dropLast(1).mapNotNull { it.weightLb }.maxOrNull()
        ).maxOrNull(),
        durationSeconds = latest.durationSeconds
    )
    val prescription = computeRestPrescription(plan, effort, ui.restTimerOverrideSeconds, performed)
    val delta = prescription.seconds - open.plannedSeconds
    if (delta != 0) {
        restTimer.addSeconds(delta)
        if (timer.isPaused) restTimer.pause()
    }
    openRestEvent = open.copy(plannedSeconds = prescription.seconds)
    _state.update { it.copy(restTimerReason = prescription.reason) }
}
