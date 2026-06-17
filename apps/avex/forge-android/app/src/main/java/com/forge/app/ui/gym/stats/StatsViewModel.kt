package com.forge.app.ui.gym.stats

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.AdaptationRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.ui.gym.stats.state.InsightFlag
import com.forge.app.ui.gym.stats.state.StatsUiState
import com.forge.app.ui.gym.stats.state.balanceRatioUi
import com.forge.app.ui.gym.stats.state.buildReadinessPulse
import com.forge.app.ui.gym.stats.state.plateauFlagOf
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class StatsViewModel @Inject constructor(
    statsRepo: StatsRepository,
    adaptationRepo: AdaptationRepository,
    settingsRepo: com.forge.app.data.prefs.SettingsRepository,
    private val bodyweightRepo: com.forge.app.data.repo.BodyweightRepository
) : ViewModel() {

    /**
     * The engine read is a whole-history snapshot fan-out — loaded ONCE per Stats open
     * (the TrophiesViewModel pattern), never inside the reactive combine below, so it
     * can't join the per-set hot path. Carries the insight observations too, so a single
     * snapshot feeds the pulse, plateaus, ratios, AND insights (was a second snapshot run
     * inside StatsRepository's combine on every emission).
     */
    private val engineFlow = MutableStateFlow<AdaptationRepository.EngineStatsRead?>(null)

    /** Whether the Body-tab quick-log should offer "Import from Health Connect" (read granted). */
    private val _weightConnected = MutableStateFlow(false)
    val weightConnected: StateFlow<Boolean> = _weightConnected.asStateFlow()

    /** Transient confirmation/result line for the quick-log sheet; cleared when the sheet closes. */
    private val _bodyweightMessage = MutableStateFlow<String?>(null)
    val bodyweightMessage: StateFlow<String?> = _bodyweightMessage.asStateFlow()

    init {
        // Guard the whole-history snapshot fan-out: a failure here leaves the engine read null
        // (no pulse/plateaus/insights) instead of an uncaught crash in viewModelScope.
        viewModelScope.launch { engineFlow.value = runCatching { adaptationRepo.engineStatsRead() }.getOrNull() }
        viewModelScope.launch {
            _weightConnected.value = runCatching { bodyweightRepo.canImportFromHealthConnect() }.getOrDefault(false)
        }
    }

    /** Save a typed weigh-in (lb); the bodyweight trend updates reactively via observeRecent. */
    fun logBodyweight(weightLb: Double) = viewModelScope.launch {
        bodyweightRepo.log(weightLb)
        _bodyweightMessage.value = "Saved."
    }

    /** Pull the latest weight from Health Connect into the log (no-op if nothing newer). */
    fun importBodyweight() = viewModelScope.launch {
        val imported = bodyweightRepo.importLatestFromHealthConnect()
        _bodyweightMessage.value =
            if (imported != null) "Imported your latest weight." else "No newer weight in Health Connect."
    }

    fun clearBodyweightMessage() { _bodyweightMessage.value = null }

    /** Re-check HC read permission (e.g. right before showing the quick-log sheet) so a grant made in
     *  Settings after this screen opened surfaces the import option without recreating the ViewModel. */
    fun refreshWeightConnected() = viewModelScope.launch {
        _weightConnected.value = runCatching { bodyweightRepo.canImportFromHealthConnect() }.getOrDefault(false)
    }

    val state: StateFlow<StatsUiState> = combine(
        statsRepo.observeGymStats(),
        engineFlow,
        settingsRepo.userSex
    ) { snapshot, engine, sex ->
        StatsUiState(
            isLoading = false,
            volumeByMuscle = snapshot.volumeByMuscle,
            recentPrs = snapshot.recentPrs,
            hallOfFame = snapshot.hallOfFame,
            exerciseHistory = snapshot.exerciseHistory,
            exerciseFrequency = snapshot.exerciseFrequency,
            timeToPr = snapshot.timeToPr,
            effortDistribution = snapshot.effortDistribution,
            prsByDayOfWeek = snapshot.prsByDayOfWeek,
            dayTypeBestVsAvg = snapshot.dayTypeBestVsAvg,
            weekComparison = snapshot.weekComparison,
            prSessionTimestamps = snapshot.prSessionTimestamps,
            insights = engine?.insights.orEmpty().map { InsightFlag(it.icon, it.title, it.body) },
            lifetimeMetrics = snapshot.lifetimeMetrics,
            moodOverTime = snapshot.moodOverTime,
            weekActivity = snapshot.weekActivity,
            thisWeekCardioMin = snapshot.thisWeekCardioMin,
            e1rmLifts = snapshot.e1rmLifts,
            repMaxes = snapshot.repMaxes,
            weeklySetsByMuscle = snapshot.weeklySetsByMuscle,
            repRangeDist = snapshot.repRangeDist,
            rpeDistribution = snapshot.rpeDistribution,
            avgRpe = snapshot.avgRpe,
            bodyweightPoints = snapshot.bodyweightPoints,
            consistencyStreakWeeks = snapshot.consistencyStreakWeeks,
            progressiveOverloadPct = snapshot.progressiveOverloadPct,
            avgRpePerSession = snapshot.avgRpePerSession,
            weeklySessionCounts = snapshot.weeklySessionCounts,
            overload = snapshot.overload,
            prRecency = snapshot.prRecency,
            patternRadar = snapshot.patternRadar,
            plannedSetsByMuscle = snapshot.plannedSetsByMuscle,
            weeklyTonnage = snapshot.weeklyTonnage,
            trainingTimes = snapshot.trainingTimes,
            weeklyDurations = snapshot.weeklyDurations,
            readinessPulse = engine?.fatigue?.let { buildReadinessPulse(it, engine.deloadScoreThreshold) },
            plateauFlags = engine?.plateaus.orEmpty().mapNotNull(::plateauFlagOf),
            balanceRatios = engine?.ratios.orEmpty().map(::balanceRatioUi),
            userSex = sex
        )
    }.catch {
        // A crash in any stats aggregation drops to a non-loading ERROR state instead of an
        // infinite spinner OR a silent empty (which would read as "no data yet"). The screen
        // shows a distinct "couldn't load your stats" message (E4).
        emit(StatsUiState(isLoading = false, loadError = true))
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = StatsUiState()
    )
}
