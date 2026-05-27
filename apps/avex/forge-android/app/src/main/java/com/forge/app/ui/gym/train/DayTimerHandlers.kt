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
            restTimer.stop()
            _state.update { it.copy(showTimerControls = false) }
        }
        is DayUiEvent.RestTimerAddSeconds -> restTimer.addSeconds(event.seconds)
        else -> {}
    }
}
