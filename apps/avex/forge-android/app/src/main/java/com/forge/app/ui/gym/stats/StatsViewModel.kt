package com.forge.app.ui.gym.stats

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.repo.AdaptationRepository
import com.forge.app.data.repo.CardioRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.ui.common.DayLog
import com.forge.app.ui.common.loadDayLog
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
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.mapLatest
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.time.LocalDate
import javax.inject.Inject

@HiltViewModel
class StatsViewModel @Inject constructor(
    statsRepo: StatsRepository,
    adaptationRepo: AdaptationRepository,
    private val settingsRepo: com.forge.app.data.prefs.SettingsRepository,
    private val sessionDao: SessionDao,
    private val cardioRepo: CardioRepository,
    engineInputs: com.forge.app.data.repo.EngineInputSignals,
    private val healthConnect: com.forge.app.data.health.HealthConnectManager,
    private val clock: com.forge.app.core.time.Clock,
    vacationRepo: com.forge.app.data.repo.VacationRepository,
    coachDao: com.forge.app.data.db.dao.CoachDao
) : ViewModel() {

    private val _body = MutableStateFlow<StatsBodyUi?>(null)
    /** The Body lens: the watch's last [BODY_DAYS] days. Null until the first read. */
    val body: StateFlow<StatsBodyUi?> = _body.asStateFlow()

    private val bodyRefresh = com.forge.app.core.ConflatedRefresh(viewModelScope) {
        runCatching { loadBody() }.onFailure {
            if (it is kotlinx.coroutines.CancellationException) throw it
            _body.value = StatsBodyUi(available = healthConnect.isAvailable, connected = false)
        }
    }

    /** Re-read Health Connect: on opening the Body lens and on resume, since nothing observes it. */
    fun refreshBody() = bodyRefresh.request()

    private suspend fun loadBody() {
        if (!healthConnect.isAvailable) {
            _body.value = StatsBodyUi(available = false, connected = false)
            return
        }
        val granted = healthConnect.grantedPermissions()
        val reads = healthConnect.permissions + healthConnect.hrvPermissions + healthConnect.stepsPermissions
        if (granted.none { it in reads }) {
            _body.value = StatsBodyUi(available = true, connected = false)
            return
        }
        val zone = java.time.ZoneId.systemDefault()
        val now = clock.nowMs()
        val today = java.time.Instant.ofEpochMilli(now).atZone(zone).toLocalDate()
        val todayStartMs = today.atStartOfDay(zone).toInstant().toEpochMilli()
        val sinceMs = today.minusDays(BODY_DAYS.toLong()).atStartOfDay(zone).toInstant().toEpochMilli()
        val snap = healthConnect.readRecovery(sinceMs, now)
        _body.value = statsBodySeries(
            snap,
            todayStartMs,
            dayStartMs = { ms ->
                java.time.Instant.ofEpochMilli(ms).atZone(zone).toLocalDate().atStartOfDay(zone).toInstant().toEpochMilli()
            },
            stepsGranted = healthConnect.stepsPermissions.all { it in granted }
        )
    }

    /** Refresh for changed engine inputs, including same-count edits and date boundaries. */
    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    private val engineFlow: StateFlow<AdaptationRepository.EngineStatsRead?> =
        engineInputs.changes()
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
    private val _dayDetail = MutableStateFlow<DayLog?>(null)
    val dayDetail: StateFlow<DayLog?> = _dayDetail.asStateFlow()

    /** The in-flight [openDay] load, cancelled by the next tap. */
    private var dayDetailJob: kotlinx.coroutines.Job? = null

    /**
     * Load everything logged on [date] (gym sessions + cardio, newest first) and open the day sheet.
     *
     * Each tap CANCELS the previous load. Every heatmap tap used to launch its own uncancelled pair
     * of queries and publish whatever it found, so two taps in quick succession raced: the slower —
     * and therefore usually the EARLIER — request wrote last, and the sheet showed a different day
     * from the cell the user had just pressed. A day with a lot logged in it is the slow one, which
     * makes the wrong answer the interesting-looking one.
     */
    fun openDay(date: LocalDate) {
        dayDetailJob?.cancel()
        dayDetailJob = viewModelScope.launch {
        runCatching {
            loadDayLog(sessionDao, cardioRepo, date)
        }.getOrElse { e ->
            // runCatching swallows CancellationException too, and a cancelled load publishing its
            // half-finished result is the very thing the cancel exists to prevent.
            if (e is kotlinx.coroutines.CancellationException) throw e else null
        }?.let { _dayDetail.value = it }
        }
    }

    fun closeDay() {
        dayDetailJob?.cancel()
        _dayDetail.value = null
    }

    /**
     * The heatmap's window of planned days off: holidays and the rest days a weekday schedule
     * planned, over the same 26 weeks the heatmap shows (the streak already bridges both).
     */
    private val restDays = combine(vacationRepo.observeAll(), settingsRepo.scheduleHistory) { periods, history ->
        val onVacation = com.forge.app.domain.vacation.VacationCalendar.onVacation(periods)
        val today = java.time.Instant.ofEpochMilli(clock.nowMs()).atZone(java.time.ZoneId.systemDefault()).toLocalDate()
        (0L until 26L * 7).mapNotNullTo(HashSet()) { back ->
            val d = today.minusDays(back)
            d.toEpochDay().takeIf {
                onVacation(d) || com.forge.app.domain.schedule.ScheduleHistory.isPlannedRest(history, d)
            }
        }
    }.flowOn(kotlinx.coroutines.Dispatchers.Default)

    val state: StateFlow<StatsUiState> = combine(
        statsRepo.observeGymStats(),
        engineFlow,
        settingsRepo.userSex,
        restDays,
        coachDao.observeOpenCallTargets()
    ) { snapshot, engine, sex, rest, openCalls ->
        StatsUiState(
            isLoading = false,
            recentPrs = snapshot.recentPrs,
            e1rmLifts = snapshot.e1rmLifts,
            strengthCurves = snapshot.strengthCurves,
            weeklySetsByMuscle = snapshot.weeklySetsByMuscle,
            plannedSetsByMuscle = snapshot.plannedSetsByMuscle,
            weeklyTonnage = snapshot.weeklyTonnage,
            balanceRatios = engine?.ratios.orEmpty().map(::balanceRatioUi),
            plateauFlags = engine?.plateaus.orEmpty().mapNotNull(::plateauFlagOf).associateBy { it.exerciseId },
            coachCallLiftIds = openCalls.toSet(),
            bodyweightPoints = snapshot.bodyweightPoints,
            userSex = sex,
            readinessPulse = engine?.fatigue?.let { buildReadinessPulse(it, engine.deloadScoreThreshold) },
            readinessThreshold = engine?.deloadScoreThreshold,
            dailyActivity = snapshot.dailyActivity,
            restEpochDays = rest,
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
