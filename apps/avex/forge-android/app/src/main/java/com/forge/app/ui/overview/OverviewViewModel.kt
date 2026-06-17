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
import com.forge.app.domain.coach.AutoCoachPlanner
import com.forge.app.domain.units.formatWeight
import com.forge.app.program.ExerciseLibrary
import com.forge.app.ui.overview.state.CoachItem
import com.forge.app.ui.overview.state.CoachLearningHint
import com.forge.app.ui.overview.state.OnThisDayMemory
import com.forge.app.ui.overview.state.OverviewRecentItem
import com.forge.app.ui.overview.state.OverviewUiState
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.util.concurrent.ConcurrentHashMap
import javax.inject.Inject

@HiltViewModel
class OverviewViewModel @Inject constructor(
    private val statsRepo: StatsRepository,
    private val cardioRepo: CardioRepository,
    private val settingsRepo: SettingsRepository,
    private val trophyRepo: TrophyRepository,
    private val customizationRepo: CustomizationRepository,
    private val workoutRepo: WorkoutRepository,
    private val sessionDao: SessionDao,
    private val adaptationRepo: com.forge.app.data.repo.AdaptationRepository,
    private val coachRepo: com.forge.app.data.repo.CoachRepository,
    private val programRepo: com.forge.app.data.repo.ProgramRepository,
    private val programChangeGuard: com.forge.app.ui.common.ProgramChangeGuard
) : ViewModel() {

    private val _onThisDayMemory = MutableStateFlow<OnThisDayMemory?>(null)
    private val _coach = MutableStateFlow<List<CoachItem>>(emptyList())

    /** Sub-gate "still learning" nudge (CD-1) — null once the coach has activated. */
    private val _coachLearning = MutableStateFlow<CoachLearningHint?>(null)

    /** Sub-threshold fatigue nudge (Tier 3) — null unless the active coach is quiet but fatigue builds. */
    private val _coachFatigue = MutableStateFlow<com.forge.app.ui.overview.state.FatigueHint?>(null)

    /** "New report ready" banner (auto-coach) — null when this week's brief has been seen. */
    private val _coachBanner = MutableStateFlow<com.forge.app.data.repo.CoachBanner?>(null)
    val coachBanner: StateFlow<com.forge.app.data.repo.CoachBanner?> = _coachBanner

    /** One-time notice after a zombie/orphan active session was auto-resolved at open (E8). */
    private val _orphanNotice = MutableStateFlow<String?>(null)
    val orphanNotice: StateFlow<String?> = _orphanNotice

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
        settingsRepo.cardioWeeklyTargetMin,
        settingsRepo.useKg
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
        val useKg = args[9] as Boolean

        buildOverviewUiState(
            stats = stats,
            recentCardio = recentCardio,
            shown = shown,
            memory = memory,
            plannedDay = plannedDay,
            trophiesUnlocked = unlockedIds.size,
            distanceKm = distanceKm,
            dayVolStats = dayVolStats,
            cardioTargetMin = cardioTarget,
            useKg = useKg
        )
    }.combine(customizationRepo.observeAllDayNames()) { s, names ->
        val customName = names.firstOrNull { it.dayKey == s.nextUpDayKey }?.customName
        s.copy(customDayName = customName)
    }.combine(workoutRepo.observeActiveSession()) { s, active ->
        // Guard against a "zombie" active session whose day was removed by a program regenerate (or a
        // force-stop mid-first-gen): don't offer to resume a day that no longer exists — Program.day()
        // would silently resolve the stale key to the wrong day. An invalid key reads as no resume here.
        s.copy(activeSessionDayKey = active?.dayKey?.takeIf { it in com.forge.app.program.Program.dayKeys })
    }.combine(_coach) { s, coach ->
        s.copy(coach = coach)
    }.combine(_coachLearning) { s, hint ->
        s.copy(coachLearning = hint)
    }.combine(_coachFatigue) { s, f ->
        s.copy(coachFatigue = f)
    }.combine(settingsRepo.daysPerWeek) { s, days ->
        // The "of N target" denominator is the actual number of training days in the generated program
        // (the real weekly schedule), not a hardcoded 6 and not the raw days/week preference — the two
        // can diverge (a frozen preset, equipment-driven slot merging). Fall back to the preference
        // while the program isn't populated yet (early startup).
        val target = com.forge.app.program.Program.days.size.takeIf { it in 1..7 } ?: days.coerceIn(1, 7)
        s.copy(weeklyWorkoutTarget = target)
    }.combine(settingsRepo.useKg) { s, useKg ->
        // Attach each recent gym row's marquee lift (its heaviest set). Each lookup is a real DB
        // read, so withTopLifts memoizes by session: a finished session's top set is immutable.
        // Without this the read re-ran on EVERY emission of the combine chain above (active-session,
        // coach, weekly-stats ticks) for the same unchanged sessions. useKg is part of the cache
        // signature so flipping the unit re-formats the marquee instead of serving a stale string.
        s.copy(recentItems = withTopLifts(s.recentItems, useKg))
    }.flowOn(Dispatchers.Default).stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = OverviewUiState()
    )

    init {
        viewModelScope.launch { _onThisDayMemory.value = statsRepo.findOnThisDayMemory() }
        viewModelScope.launch { reloadCoach() }
        // Backfill the first-touch flag for users who already have history, so the onboarding cards
        // never reappear for a returning user (e.g. after a data wipe). finishWorkout() sets it going forward.
        viewModelScope.launch {
            if (statsRepo.observeWeeklyStats().first().totalFinishedSessions > 0) settingsRepo.setFirstWorkoutDone()
        }
        // First open of a new week triggers the Weekly Coach Pass (idempotent by week id) and
        // surfaces the banner only if this week's brief hasn't been seen. The repo stamps errors
        // as their own pass status; this guard only protects Overview.
        viewModelScope.launch {
            runCatching { coachRepo.pendingBanner() }
                .onSuccess { _coachBanner.value = it }
        }
        // Detect + resolve a zombie active session (force-stop / regenerate artifact) so it can't drive
        // a misleading resume or block a fresh start; surface what happened once (E8). Await ensureLoaded
        // FIRST: Program.dayKeys reports the hard-coded SEED split until the DB program is loaded, so
        // resolving against it on a cold start could finish a perfectly valid session whose day simply
        // isn't in the seed. ensureLoaded is idempotent + serialized, so this just waits out startup.
        viewModelScope.launch {
            runCatching {
                programRepo.ensureLoaded()
                workoutRepo.resolveOrphanSession(com.forge.app.program.Program.dayKeys.toSet())
            }.getOrNull()?.let { res ->
                _orphanNotice.value = if (res.finishedToHistory)
                    "Saved an unfinished workout from a day that's no longer in your program — it's in your history now."
                else
                    "Cleared an empty leftover session from a workout day that's no longer in your program."
            }
        }
    }

    /** Dismiss the "new report" banner without opening the brief — still marks it seen. */
    fun dismissCoachBanner() {
        val weekId = _coachBanner.value?.weekId ?: return
        _coachBanner.value = null
        viewModelScope.launch { runCatching { coachRepo.markSeen(weekId) } }
    }

    /** Dismiss the one-time orphan-session notice (E8). */
    fun dismissOrphanNotice() { _orphanNotice.value = null }

    // ─── Coach feed (adaptation engine) ───────────────────────────────────────

    private suspend fun reloadCoach() {
        val feed = adaptationRepo.coachFeed()
        val recs = feed.recommendations
            .mapNotNull { it.toCoachItem() }
            .take(3)
        _coach.value = recs
        // CD-1: only nudge "still learning" when there's nothing actionable AND the weekly pass
        // hasn't activated yet (below MIN_SESSIONS). Cheap finished-count query, off the hot path.
        val logged = sessionDao.finishedCount()
        _coachLearning.value = if (recs.isEmpty() && logged < AutoCoachPlanner.MIN_SESSIONS)
            CoachLearningHint(logged, AutoCoachPlanner.MIN_SESSIONS - logged) else null
        // Tier 3: when the coach is active but quiet, surface building fatigue (System 5 sub-threshold).
        _coachFatigue.value = if (recs.isEmpty() && logged >= AutoCoachPlanner.MIN_SESSIONS) {
            feed.fatigueBuilding?.let {
                com.forge.app.ui.overview.state.FatigueHint(it.score, feed.fatigueThreshold, it.drivers.firstOrNull())
            }
        } else null
    }

    /**
     * One-tap apply. Only the deload suggestion has one today; the rest apply in-session.
     * The deload regenerates the program, which discards any in-progress workout — route it through
     * the guard so that's confirmed (and reloaded) rather than wiped silently.
     */
    fun applyCoach(item: CoachItem) = viewModelScope.launch {
        if (item.id == "deload.suggest") programChangeGuard.run { adaptationRepo.applyDeloadWeek(); reloadCoach() }
        else reloadCoach()
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

    /**
     * Marquee lift per gym row, memoized by sessionId → (volume+unit signature, lift). A finished
     * session's heaviest set is immutable, so it's computed once; the signature busts the entry when
     * the session is later edited (its denormalized volume changes) OR the user toggles kg/lb. Cardio
     * rows and unsaved items pass through unchanged. Accessed only from the single flow collector
     * above, so writes are already serial; kept a ConcurrentHashMap so a future parallelization of the
     * per-item reads can't introduce a silent data race.
     */
    private val topLiftCache = ConcurrentHashMap<Long, Pair<Pair<Double?, Boolean>, String?>>()

    private suspend fun withTopLifts(items: List<OverviewRecentItem>, useKg: Boolean): List<OverviewRecentItem> =
        items.map { item ->
            if (!item.isGym || item.id < 0) return@map item
            val sig = item.volumeLb to useKg
            val cached = topLiftCache[item.id]
            val lift = if (cached != null && cached.first == sig) cached.second
                else topLiftFor(item.id, useKg).also { topLiftCache[item.id] = sig to it }
            item.copy(topLift = lift)
        }

    /** Heaviest weighted set of a session, formatted "Name 185 lb × 5". Null for bodyweight-only days. */
    private suspend fun topLiftFor(sessionId: Long, useKg: Boolean): String? {
        val top = statsRepo.getSessionExerciseLines(sessionId)
            .filter { (it.topWeightLb ?: 0.0) > 0.0 }
            .maxByOrNull { it.topWeightLb!! } ?: return null
        val wText = formatWeight(top.topWeightLb!!, useKg)
        return "${top.exerciseName} $wText × ${top.topReps ?: 0}"
    }
}
