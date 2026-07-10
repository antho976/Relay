package com.forge.app.ui.gym.stats

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.repo.AdaptationRepository
import com.forge.app.data.repo.CardioRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.ui.gym.history.HistoryItem
import com.forge.app.ui.gym.stats.state.StatsUiState
import com.forge.app.ui.gym.stats.state.balanceRatioUi
import com.forge.app.ui.gym.stats.state.buildReadinessPulse
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.mapLatest
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.time.ZoneId
import javax.inject.Inject

/**
 * Everything logged on one calendar day — opened by tapping a lit day on the consistency heatmap.
 * Reuses [HistoryItem] so the sheet renders the exact same rows as the History screen.
 */
data class StatsDayDetail(
    val date: LocalDate,
    val items: List<HistoryItem>
)

@HiltViewModel
class StatsViewModel @Inject constructor(
    statsRepo: StatsRepository,
    adaptationRepo: AdaptationRepository,
    private val settingsRepo: com.forge.app.data.prefs.SettingsRepository,
    private val sessionDao: SessionDao,
    private val cardioRepo: CardioRepository
) : ViewModel() {

    /**
     * The engine read is a whole-history snapshot fan-out. Re-run it whenever the set of FINISHED
     * sessions changes — a session finishes, or "Reset session data" / a delete wipes them — by
     * keying off the finished-session COUNT, NOT the per-set reactive combine below, so it still
     * can't join the per-set hot path (logging sets in an in-progress session leaves the count,
     * and so this flow, untouched). A one-shot init load instead left the always-on balance ratios
     * (push/pull, quad/ham) stale on the retained ViewModel after a reset — GYMAP-18. runCatching
     * degrades a snapshot failure to null (no pulse/plateaus/insights) rather than crashing.
     */
    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    private val engineFlow: StateFlow<AdaptationRepository.EngineStatsRead?> =
        sessionDao.observeFinishedCount()
            .mapLatest {
                // Degrade a real snapshot failure to null (no pulse/plateaus/insights), but let a
                // mapLatest cancellation propagate — swallowing it could emit a stale null.
                runCatching { adaptationRepo.engineStatsRead() }
                    .getOrElse { if (it is kotlinx.coroutines.CancellationException) throw it else null }
            }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    /** Last Stats LENS the user settled on (by enum NAME), persisted so reopening Stats lands there
     *  (reuses the old sub-tab pref; retired tab names simply fall back to the default). Maps an
     *  unset pref to the default so emitted values are always non-null — the screen reserves null
     *  for "not yet loaded" and only deep-links once a real value arrives. */
    val lastStatsTabName: StateFlow<String?> =
        settingsRepo.lastStatsTabName
            .map { it ?: StatsLens.STRENGTH.name }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    fun saveStatsTab(name: String) = viewModelScope.launch { settingsRepo.setLastStatsTabName(name) }

    /** The day the user tapped on the consistency heatmap; null = sheet closed. */
    private val _dayDetail = MutableStateFlow<StatsDayDetail?>(null)
    val dayDetail: StateFlow<StatsDayDetail?> = _dayDetail.asStateFlow()

    /** Load everything logged on [date] (gym sessions + cardio, newest first) and open the day sheet. */
    fun openDay(date: LocalDate) = viewModelScope.launch {
        runCatching {
            val zone = ZoneId.systemDefault()
            val fromMs = date.atStartOfDay(zone).toInstant().toEpochMilli()
            val toMs = date.plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli()
            val workouts = sessionDao.finishedInRange(fromMs, toMs).map { HistoryItem.Workout(it) }
            // Bounded query (non-rest, in [fromMs, toMs)) instead of loading the whole cardio history
            // and filtering one day out of it.
            val cardio = cardioRepo.entriesInRange(fromMs, toMs).map { HistoryItem.Cardio(it) }
            StatsDayDetail(date, (workouts + cardio).sortedByDescending { it.dateMs })
        }.getOrNull()?.let { _dayDetail.value = it }
    }

    fun closeDay() { _dayDetail.value = null }

    val state: StateFlow<StatsUiState> = combine(
        statsRepo.observeGymStats(),
        engineFlow,
        settingsRepo.userSex
    ) { snapshot, engine, sex ->
        StatsUiState(
            isLoading = false,
            recentPrs = snapshot.recentPrs,
            e1rmLifts = snapshot.e1rmLifts,
            strengthCurves = snapshot.strengthCurves,
            weeklySetsByMuscle = snapshot.weeklySetsByMuscle,
            plannedSetsByMuscle = snapshot.plannedSetsByMuscle,
            weeklyTonnage = snapshot.weeklyTonnage,
            balanceRatios = engine?.ratios.orEmpty().map(::balanceRatioUi),
            bodyweightPoints = snapshot.bodyweightPoints,
            userSex = sex,
            readinessPulse = engine?.fatigue?.let { buildReadinessPulse(it, engine.deloadScoreThreshold) },
            readinessThreshold = engine?.deloadScoreThreshold,
            dailyActivity = snapshot.dailyActivity,
            rpeDistribution = snapshot.rpeDistribution,
            avgRpe = snapshot.avgRpe,
            trainingTimes = snapshot.trainingTimes,
            prsByDayOfWeek = snapshot.prsByDayOfWeek,
            weekComparison = snapshot.weekComparison,
            hallOfFame = snapshot.hallOfFame,
            lifetime = snapshot.lifetime
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
