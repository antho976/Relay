package com.forge.app.ui.overview

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.CardioRepository
import com.forge.app.data.repo.CustomizationRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.data.repo.TrophyRepository
import com.forge.app.ui.overview.state.OnThisDayMemory
import com.forge.app.ui.overview.state.OverviewRecentItem
import com.forge.app.ui.overview.state.OverviewUiState
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class OverviewViewModel @Inject constructor(
    private val statsRepo: StatsRepository,
    private val cardioRepo: CardioRepository,
    private val settingsRepo: SettingsRepository,
    private val trophyRepo: TrophyRepository,
    private val customizationRepo: CustomizationRepository
) : ViewModel() {

    private val _onThisDayMemory = MutableStateFlow<OnThisDayMemory?>(null)

    private val weekStartMs = System.currentTimeMillis() - 7L * 24 * 60 * 60 * 1000

    val state: StateFlow<OverviewUiState> = combine(
        statsRepo.observeWeeklyStats(),
        cardioRepo.observeRecent(7),
        settingsRepo.lastDeloadAtSessionCount,
        settingsRepo.shownMilestones,
        _onThisDayMemory,
        settingsRepo.plannedNextDay,
        trophyRepo.observeUnlockedIds(),
        cardioRepo.observeDistanceKmSince(weekStartMs),
        statsRepo.observeDayVolumeStats(),
        settingsRepo.cardioWeeklyTargetMin
    ) { args ->
        val stats = args[0] as StatsRepository.WeeklyStats
        @Suppress("UNCHECKED_CAST")
        val recentCardio = args[1] as List<com.forge.app.data.db.entities.CardioEntry>
        val lastDeload = args[2] as Int
        @Suppress("UNCHECKED_CAST")
        val shown = args[3] as Set<String>
        val memory = args[4] as OnThisDayMemory?
        val plannedDay = args[5] as String
        val unlockedIds = args[6] as List<*>
        val distanceKm = (args[7] as Double?) ?: 0.0
        @Suppress("UNCHECKED_CAST")
        val dayVolStats = args[8] as Map<String, SessionDao.DayVolumeStats>
        val cardioTarget = args[9] as Int

        buildOverviewUiState(
            stats = stats,
            recentCardio = recentCardio,
            lastDeload = lastDeload,
            shown = shown,
            memory = memory,
            plannedDay = plannedDay,
            trophiesUnlocked = unlockedIds.size,
            distanceKm = distanceKm,
            dayVolStats = dayVolStats,
            cardioTargetMin = cardioTarget
        )
    }.combine(customizationRepo.observeAllDayNames()) { s, names ->
        val customName = names.firstOrNull { it.dayKey == s.nextUpDayKey }?.customName
        s.copy(customDayName = customName)
    }.flowOn(Dispatchers.Default).stateIn(
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

    /** Consume the "Train X today" override once a session is started, so it reverts to rotation. */
    fun onSessionStarting() = viewModelScope.launch {
        if (state.value.plannedNextDay.isNotBlank()) settingsRepo.setPlannedNextDay("")
    }

    private val _selectedItem = MutableStateFlow<OverviewRecentItem?>(null)
    val selectedItem: StateFlow<OverviewRecentItem?> = _selectedItem

    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    val sessionExerciseLines: StateFlow<List<StatsRepository.SessionExerciseLine>> =
        _selectedItem.flatMapLatest { item ->
            if (item == null || !item.isGym || item.id < 0) flowOf(emptyList())
            else flow { emit(statsRepo.getSessionExerciseLines(item.id)) }
        }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun selectRecentItem(item: OverviewRecentItem) { _selectedItem.value = item }
    fun clearSelectedItem() { _selectedItem.value = null }
}
