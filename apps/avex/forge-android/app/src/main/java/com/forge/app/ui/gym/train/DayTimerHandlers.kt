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
    val skipped: Boolean = false
)

/** Close (and persist) the open rest interval — the set logged at [endedAtMs] ended it. */
internal suspend fun DayViewModel.closeOpenRestEvent(sessionId: Long, endedAtMs: Long) {
    val open = openRestEvent ?: return
    openRestEvent = null
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
