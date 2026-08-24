package com.forge.app.ui.gym.train

import androidx.lifecycle.viewModelScope
import com.forge.app.ui.gym.train.state.DayUiEvent
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.ZoneId
import java.time.temporal.TemporalAdjusters

internal fun DayViewModel.handleWarmupEvent(event: DayUiEvent) {
    when (event) {
        is DayUiEvent.ToggleWarmupStep -> _state.update { current ->
            val next = current.warmupChecked.toMutableSet()
            if (!next.add(event.id)) next.remove(event.id)
            current.copy(warmupChecked = next)
        }
        // Starting and skipping were two buttons doing the same thing. There is one now, and the
        // ticks are the user's own scratchpad: the app cannot know whether the arm circles happened,
        // so it neither gates on them nor stores an outcome.
        is DayUiEvent.CompleteWarmup, is DayUiEvent.SkipWarmup ->
            _state.update { it.copy(isWarmupComplete = true) }
        is DayUiEvent.DisableWarmupToday -> viewModelScope.launch {
            val untilMs = LocalDate.now().plusDays(1)
                .atStartOfDay(ZoneId.systemDefault()).toInstant().toEpochMilli()
            settingsRepo.setWarmupDisabledUntilMs(untilMs)
            _state.update { it.copy(isWarmupComplete = true) }
        }
        is DayUiEvent.DisableWarmupWeek -> viewModelScope.launch {
            val untilMs = LocalDate.now()
                .with(TemporalAdjusters.next(DayOfWeek.MONDAY))
                .atStartOfDay(ZoneId.systemDefault()).toInstant().toEpochMilli()
            settingsRepo.setWarmupDisabledUntilMs(untilMs)
            _state.update { it.copy(isWarmupComplete = true) }
        }
        else -> {}
    }
}
