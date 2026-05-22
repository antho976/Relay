package com.forge.app.ui.gym.stats

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.StatsRepository
import com.forge.app.ui.gym.stats.state.StatsUiState
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

@HiltViewModel
class StatsViewModel @Inject constructor(
    statsRepo: StatsRepository
) : ViewModel() {

    val state: StateFlow<StatsUiState> = statsRepo.observeGymStats()
        .map { snapshot ->
            StatsUiState(
                isLoading = false,
                totals = snapshot.totals,
                heatmap = snapshot.heatmap,
                volumeByMuscle = snapshot.volumeByMuscle,
                strengthCurve = snapshot.strengthCurve,
                recentPrs = snapshot.recentPrs
            )
        }
        .stateIn(
            scope = viewModelScope,
            started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
            initialValue = StatsUiState()
        )
}
