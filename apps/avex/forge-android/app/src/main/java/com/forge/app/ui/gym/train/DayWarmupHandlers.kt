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
            // Size the checks list to the ACTUAL warmup item count (it defaults to 4). Otherwise a
            // custom warmup with fewer than 4 items can never be completed, and one with more than
            // 4 items auto-completes early once the first four are checked.
            val itemCount = (current.customWarmupItems ?: current.dayPlan.warmup).size
            val checks = MutableList(itemCount) { current.warmupChecks.getOrElse(it) { false } }
            if (event.index in 0 until itemCount) checks[event.index] = !checks[event.index]
            current.copy(
                warmupChecks = checks,
                isWarmupComplete = checks.isNotEmpty() && checks.all { it }
            )
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
