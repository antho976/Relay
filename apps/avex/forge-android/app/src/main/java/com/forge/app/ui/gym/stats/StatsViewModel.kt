package com.forge.app.ui.gym.stats

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.AdaptationRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.ui.gym.stats.state.StatsUiState
import com.forge.app.ui.gym.stats.state.balanceRatioUi
import com.forge.app.ui.gym.stats.state.buildReadinessPulse
import com.forge.app.ui.gym.stats.state.plateauFlagOf
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class StatsViewModel @Inject constructor(
    statsRepo: StatsRepository,
    adaptationRepo: AdaptationRepository
) : ViewModel() {

    /**
     * The engine read is a whole-history snapshot fan-out — loaded ONCE per Stats open
     * (the TrophiesViewModel pattern), never inside the reactive combine below, so it
     * can't join the per-set hot path.
     */
    private val engineFlow = MutableStateFlow<AdaptationRepository.EngineStatsRead?>(null)

    init {
        viewModelScope.launch { engineFlow.value = adaptationRepo.engineStatsRead() }
    }

    val state: StateFlow<StatsUiState> = combine(
        statsRepo.observeGymStats(),
        engineFlow
    ) { snapshot, engine ->
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
            insights = snapshot.insights,
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
            balanceRatios = engine?.ratios.orEmpty().map(::balanceRatioUi)
        )
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = StatsUiState()
    )
}
