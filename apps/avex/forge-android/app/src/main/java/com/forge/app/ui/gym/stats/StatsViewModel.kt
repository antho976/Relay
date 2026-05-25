package com.forge.app.ui.gym.stats

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.RestDayRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.ui.gym.stats.state.StatsUiState
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class StatsViewModel @Inject constructor(
    statsRepo: StatsRepository,
    private val restDayRepo: RestDayRepository
) : ViewModel() {

    val state: StateFlow<StatsUiState> = combine(
        statsRepo.observeGymStats(),
        statsRepo.observeMonthCalendar()
    ) { snapshot, calendar ->
        StatsUiState(
            isLoading = false,
            totals = snapshot.totals,
            heatmap = snapshot.heatmap,
            volumeByMuscle = snapshot.volumeByMuscle,
            strengthCurve = snapshot.strengthCurve,
            recentPrs = snapshot.recentPrs,
            hallOfFame = snapshot.hallOfFame,
            exerciseHistory = snapshot.exerciseHistory,
            monthCalendar = calendar,
            exerciseFrequency = snapshot.exerciseFrequency,
            timeToPr = snapshot.timeToPr,
            effortDistribution = snapshot.effortDistribution,
            prsByDayOfWeek = snapshot.prsByDayOfWeek,
            volumeDeloadTrend = snapshot.volumeDeloadTrend,
            dayTypeBestVsAvg = snapshot.dayTypeBestVsAvg,
            weekComparison = snapshot.weekComparison,
            monthComparison = snapshot.monthComparison,
            exerciseYoY = snapshot.exerciseYoY,
            exerciseVolumeHistory = snapshot.exerciseVolumeHistory,
            compoundMaxes = snapshot.compoundMaxes,
            prSessionTimestamps = snapshot.prSessionTimestamps,
            insights = snapshot.insights,
            dayTypeBreakdown = snapshot.dayTypeBreakdown,
            lifetimeMetrics = snapshot.lifetimeMetrics,
            moodOverTime = snapshot.moodOverTime,
            weekActivity = snapshot.weekActivity,
            thisWeekCardioMin = snapshot.thisWeekCardioMin
        )
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = StatsUiState()
    )

    fun markRestDay(dateKey: String, type: String) =
        viewModelScope.launch { restDayRepo.markRestDay(dateKey, type) }
    fun clearRestDay(dateKey: String) =
        viewModelScope.launch { restDayRepo.clearRestDay(dateKey) }
}
