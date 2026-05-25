package com.forge.app.ui.overview

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.CardioRepository
import com.forge.app.data.repo.CustomizationRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.data.repo.TrophyRepository
import com.forge.app.program.Program
import com.forge.app.ui.overview.state.MilestoneEvent
import com.forge.app.ui.overview.state.OnThisDayMemory
import com.forge.app.ui.overview.state.OverviewRecentItem
import com.forge.app.ui.overview.state.OverviewUiState
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.TextStyle
import java.util.Locale
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
        statsRepo.observeDayVolumeStats()
    ) { args ->
        val stats = args[0] as StatsRepository.WeeklyStats
        @Suppress("UNCHECKED_CAST")
        val recentCardio = args[1] as List<com.forge.app.data.db.entities.CardioEntry>
        val lastDeload = args[2] as Int
        val shown = args[3] as Set<*>
        val memory = args[4] as OnThisDayMemory?
        val plannedDay = args[5] as String
        @Suppress("UNCHECKED_CAST")
        val unlockedIds = args[6] as List<*>
        val distanceKm = (args[7] as Double?) ?: 0.0
        @Suppress("UNCHECKED_CAST")
        val dayVolStats = args[8] as Map<String, SessionDao.DayVolumeStats>

        val gymItems = stats.recentGymSessions.map { session ->
            val day = Program.days.firstOrNull { it.key == session.dayKey }
            val durationMin = session.finishedAt?.let { ((it - session.startedAt) / 60_000).toInt() }
            val exCount = day?.exercises?.size ?: 0
            val sub = listOfNotNull(
                if (exCount > 0) "$exCount ex" else null,
                durationMin?.let { "${it} min" }
            ).joinToString(" · ")
            val volStats = dayVolStats[session.dayKey]
            val vsAvgPct = if (volStats != null && session.totalVolumeLb != null && volStats.avgVolume > 0)
                (((session.totalVolumeLb - volStats.avgVolume) / volStats.avgVolume) * 100).toInt()
            else null
            val isBest = volStats != null && session.totalVolumeLb != null &&
                session.totalVolumeLb >= volStats.maxVolume
            Pair(session.startedAt, OverviewRecentItem(
                dayLabel = relativeDay(session.startedAt),
                title = day?.defaultName ?: session.dayKey,
                subtitle = sub,
                tag = day?.word ?: "",
                id = session.id,
                timestampMs = session.startedAt,
                isGym = true,
                volumeLb = session.totalVolumeLb,
                prCount = session.prCount,
                vsAvgPct = vsAvgPct,
                isBest = isBest,
                durationMin = durationMin
            ))
        }
        val cardioItems = recentCardio.map { entry ->
            val typeName = entry.type.replaceFirstChar { it.uppercase() }
            val sub = listOfNotNull(
                "${entry.durationMin} min",
                entry.distanceKm?.takeIf { it > 0 }?.let { "${it} km" }
            ).joinToString(" · ")
            Pair(entry.date, OverviewRecentItem(
                dayLabel = relativeDay(entry.date),
                title = "Cardio · $typeName",
                subtitle = sub,
                tag = "MOVE",
                id = entry.id,
                timestampMs = entry.date,
                isGym = false,
                durationMin = entry.durationMin,
                distanceKm = entry.distanceKm
            ))
        }
        val recentItems = (gymItems + cardioItems)
            .sortedByDescending { it.first }
            .take(2)
            .map { it.second }

        val zone2 = ZoneId.systemDefault()
        val todayLocal = LocalDate.now(zone2)
        val isoWeekStart = todayLocal.with(DayOfWeek.MONDAY)
        val cardioWeekDays = recentCardio.mapNotNull { entry ->
            val d = Instant.ofEpochMilli(entry.date).atZone(zone2).toLocalDate()
            if (!d.isBefore(isoWeekStart) && !d.isAfter(todayLocal)) d.dayOfWeek.value - 1 else null
        }.toSet()

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
            plannedNextDay = plannedDay,
            nextUpDayKey = stats.nextUpDayKey,
            weekDaysTrained = stats.weekDaysTrained,
            cardioWeekDays = cardioWeekDays,
            recentItems = recentItems,
            trophiesUnlocked = unlockedIds.size,
            cardioDistanceKm = distanceKm
        )
    }.combine(customizationRepo.observeAllDayNames()) { s, names ->
        val customName = names.firstOrNull { it.dayKey == s.nextUpDayKey }?.customName
        s.copy(customDayName = customName)
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

    private fun relativeDay(epochMs: Long): String {
        val zone = ZoneId.systemDefault()
        val date = Instant.ofEpochMilli(epochMs).atZone(zone).toLocalDate()
        val today = LocalDate.now(zone)
        return when (date) {
            today -> "TODAY"
            today.minusDays(1) -> "YESTERDAY"
            else -> date.dayOfWeek.getDisplayName(TextStyle.SHORT, Locale.getDefault()).uppercase()
        }
    }

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
