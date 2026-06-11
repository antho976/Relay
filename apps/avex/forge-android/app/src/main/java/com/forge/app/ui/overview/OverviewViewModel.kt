package com.forge.app.ui.overview

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.CardioRepository
import com.forge.app.data.repo.CustomizationRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.data.repo.TrophyRepository
import com.forge.app.data.repo.WorkoutRepository
import com.forge.app.domain.adapt.Recommendation
import com.forge.app.program.ExerciseLibrary
import com.forge.app.ui.overview.state.CoachItem
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
    private val customizationRepo: CustomizationRepository,
    private val workoutRepo: WorkoutRepository,
    private val adaptationRepo: com.forge.app.data.repo.AdaptationRepository,
    private val coachRepo: com.forge.app.data.repo.CoachRepository
) : ViewModel() {

    private val _onThisDayMemory = MutableStateFlow<OnThisDayMemory?>(null)
    private val _coach = MutableStateFlow<List<CoachItem>>(emptyList())

    /** "New report ready" banner (auto-coach) — null when this week's brief has been seen. */
    private val _coachBanner = MutableStateFlow<com.forge.app.data.repo.CoachBanner?>(null)
    val coachBanner: StateFlow<com.forge.app.data.repo.CoachBanner?> = _coachBanner

    private val weekStartMs = System.currentTimeMillis() - 7L * 24 * 60 * 60 * 1000

    val state: StateFlow<OverviewUiState> = combine(
        statsRepo.observeWeeklyStats(),
        cardioRepo.observeRecent(7),
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
        @Suppress("UNCHECKED_CAST")
        val shown = args[2] as Set<String>
        val memory = args[3] as OnThisDayMemory?
        val plannedDay = args[4] as String
        val unlockedIds = args[5] as List<*>
        val distanceKm = (args[6] as Double?) ?: 0.0
        @Suppress("UNCHECKED_CAST")
        val dayVolStats = args[7] as Map<String, SessionDao.DayVolumeStats>
        val cardioTarget = args[8] as Int

        buildOverviewUiState(
            stats = stats,
            recentCardio = recentCardio,
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
    }.combine(workoutRepo.observeActiveSession()) { s, active ->
        s.copy(activeSessionDayKey = active?.dayKey)
    }.combine(_coach) { s, coach ->
        s.copy(coach = coach)
    }.flowOn(Dispatchers.Default).stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = OverviewUiState()
    )

    init {
        viewModelScope.launch { _onThisDayMemory.value = statsRepo.findOnThisDayMemory() }
        viewModelScope.launch { reloadCoach() }
        // First open of a new week triggers the Weekly Coach Pass (idempotent by week id) and
        // surfaces the banner only if this week's brief hasn't been seen. The repo stamps errors
        // as their own pass status; this guard only protects Overview.
        viewModelScope.launch {
            runCatching { coachRepo.pendingBanner() }
                .onSuccess { _coachBanner.value = it }
        }
    }

    /** Dismiss the "new report" banner without opening the brief — still marks it seen. */
    fun dismissCoachBanner() {
        val weekId = _coachBanner.value?.weekId ?: return
        _coachBanner.value = null
        viewModelScope.launch { runCatching { coachRepo.markSeen(weekId) } }
    }

    // ─── Coach feed (adaptation engine) ───────────────────────────────────────

    private suspend fun reloadCoach() {
        _coach.value = adaptationRepo.coachRecommendations()
            .mapNotNull { it.toCoachItem() }
            .take(3)
    }

    /** One-tap apply. Only the deload suggestion has one today; the rest apply in-session. */
    fun applyCoach(item: CoachItem) = viewModelScope.launch {
        if (item.id == "deload.suggest") adaptationRepo.applyDeloadWeek()
        reloadCoach()
    }

    /** Dismissal is logged (advice_event) — the engine mutes this id for its cooldown. */
    fun dismissCoach(item: CoachItem) = viewModelScope.launch {
        adaptationRepo.logAdviceDismissed(item.id)
        reloadCoach()
    }

    private fun Recommendation.toCoachItem(): CoachItem? = when (this) {
        is Recommendation.DeloadSuggestion -> CoachItem(
            id = id, title = "Time for a deload week", body = reason,
            applyLabel = "Generate deload week"
        )
        is Recommendation.VariationSwap -> {
            val names = candidateIds.mapNotNull { ExerciseLibrary.byId(it)?.name }
            CoachItem(
                id = id, title = "Plateau: swap $exerciseName?",
                body = reason + if (names.isNotEmpty()) " Try: ${names.joinToString(", ")}." else ""
            )
        }
        is Recommendation.RepRangeShift -> CoachItem(
            id = id, title = "Shift $exerciseName to $toReps reps", body = reason
        )
        is Recommendation.WeightChange -> CoachItem(
            id = id, title = "Adjust $exerciseName to $inputText", body = reason
        )
        // Readiness scales feed the in-session chip; insights live on Stats.
        else -> null
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
