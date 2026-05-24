package com.forge.app.ui.overview

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.ui.overview.state.MilestoneEvent
import com.forge.app.ui.overview.state.OnThisDayMemory
import com.forge.app.ui.overview.state.OverviewUiState
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.time.Instant
import java.time.YearMonth
import java.time.ZoneId
import javax.inject.Inject

@HiltViewModel
class OverviewViewModel @Inject constructor(
    private val statsRepo: StatsRepository,
    private val settingsRepo: SettingsRepository
) : ViewModel() {

    private val _onThisDayMemory = MutableStateFlow<OnThisDayMemory?>(null)

    val state: StateFlow<OverviewUiState> = combine(
        statsRepo.observeWeeklyStats(),
        settingsRepo.lastDeloadAtSessionCount,
        settingsRepo.shownMilestones,
        _onThisDayMemory,
        settingsRepo.plannedNextDay
    ) { args ->
        val stats = args[0] as StatsRepository.WeeklyStats
        val lastDeload = args[1] as Int
        val shown = args[2] as Set<*>
        val memory = args[3] as OnThisDayMemory?
        val plannedDay = args[4] as String
        @Suppress("UNCHECKED_CAST")
        OverviewUiState(
            workoutsThisWeek = stats.workouts,
            volumeThisWeekLb = stats.volumeLb,
            cardioMinutesThisWeek = stats.cardioMinutes,
            totalFinishedSessions = stats.totalFinishedSessions,
            lastDeloadAtSessionCount = lastDeload,
            streakDays = stats.streakDays,
            daysSinceLastSession = stats.daysSinceLastSession,
            pendingMilestone = computePendingMilestone(stats, shown as Set<String>),
            onThisDayMemory = memory,
            plannedNextDay = plannedDay
        )
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = OverviewUiState()
    )

    init {
        viewModelScope.launch { _onThisDayMemory.value = statsRepo.findOnThisDayMemory() }
    }

    fun onMarkDeloaded() {
        viewModelScope.launch {
            settingsRepo.setLastDeloadAtSessionCount(state.value.totalFinishedSessions)
        }
    }

    fun onMilestoneShown(milestoneId: String) {
        viewModelScope.launch { settingsRepo.markMilestoneShown(milestoneId) }
    }

    fun setPlanNextDay(dayKey: String) = viewModelScope.launch {
        settingsRepo.setPlannedNextDay(dayKey)
    }
    fun clearPlanNextDay() = viewModelScope.launch { settingsRepo.clearPlannedNextDay() }

    private fun computePendingMilestone(
        stats: StatsRepository.WeeklyStats,
        shown: Set<String>
    ): MilestoneEvent? {
        if (stats.totalFinishedSessions >= 100 && MILESTONE_SESSIONS_100 !in shown) {
            return MilestoneEvent(MILESTONE_SESSIONS_100, "100 workouts complete. You've earned this.")
        }
        if (stats.volumeLb >= 10_000.0 && MILESTONE_VOLUME_10K !in shown) {
            return MilestoneEvent(MILESTONE_VOLUME_10K, "10,000 lb this week. Volume beast.")
        }
        val firstMs = stats.firstFinishedSessionMs
        if (firstMs != null && MILESTONE_FIRST_MONTH !in shown) {
            val zone = ZoneId.systemDefault()
            val firstMonth = YearMonth.from(Instant.ofEpochMilli(firstMs).atZone(zone))
            if (firstMonth < YearMonth.now(zone)) {
                return MilestoneEvent(MILESTONE_FIRST_MONTH, "First full month of training. You're building something real.")
            }
        }
        return null
    }

    companion object {
        const val MILESTONE_SESSIONS_100 = "sessions_100"
        const val MILESTONE_VOLUME_10K = "volume_10k_week"
        const val MILESTONE_FIRST_MONTH = "first_full_month"
    }
}
