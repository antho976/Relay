package com.forge.app.ui.overview

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.ui.overview.state.OverviewUiState
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class OverviewViewModel @Inject constructor(
    private val statsRepo: StatsRepository,
    private val settingsRepo: SettingsRepository
) : ViewModel() {

    val state: StateFlow<OverviewUiState> = combine(
        statsRepo.observeWeeklyStats(),
        settingsRepo.lastDeloadAtSessionCount
    ) { stats, lastDeload ->
        OverviewUiState(
            workoutsThisWeek = stats.workouts,
            volumeThisWeekLb = stats.volumeLb,
            cardioMinutesThisWeek = stats.cardioMinutes,
            totalFinishedSessions = stats.totalFinishedSessions,
            lastDeloadAtSessionCount = lastDeload
        )
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = OverviewUiState()
    )

    fun onMarkDeloaded() {
        viewModelScope.launch {
            settingsRepo.setLastDeloadAtSessionCount(state.value.totalFinishedSessions)
        }
    }
}
