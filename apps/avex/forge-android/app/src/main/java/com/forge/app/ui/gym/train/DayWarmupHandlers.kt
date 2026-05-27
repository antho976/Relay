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
        is DayUiEvent.ToggleWarmupItem -> _state.update { current ->
            val updated = current.warmupChecks.toMutableList().also {
                if (event.index in it.indices) it[event.index] = !it[event.index]
            }
            current.copy(warmupChecks = updated, isWarmupComplete = updated.all { it })
        }
        is DayUiEvent.SkipWarmup -> _state.update { it.copy(isWarmupComplete = true) }
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
