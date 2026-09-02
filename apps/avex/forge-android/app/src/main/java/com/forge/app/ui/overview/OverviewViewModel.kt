package com.forge.app.ui.overview

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.CardioRepository
import com.forge.app.data.repo.CustomizationRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.data.repo.WorkoutRepository
import com.forge.app.domain.adapt.Recommendation
import com.forge.app.domain.coach.AutoCoachPlanner
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.program.ExerciseLibrary
import com.forge.app.ui.overview.state.CoachItem
import com.forge.app.ui.overview.state.CoachLearningHint
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
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.util.concurrent.ConcurrentHashMap
import javax.inject.Inject

@HiltViewModel
class OverviewViewModel @Inject constructor(
    private val statsRepo: StatsRepository,
    private val cardioRepo: CardioRepository,
    private val settingsRepo: SettingsRepository,
    private val customizationRepo: CustomizationRepository,
    private val workoutRepo: WorkoutRepository,
    private val goalRepo: com.forge.app.data.repo.GoalRepository,
    private val extendedGoalRepo: com.forge.app.data.repo.ExtendedGoalRepository,
    private val bodyweightRepo: com.forge.app.data.repo.BodyweightRepository,
    private val sessionDao: SessionDao,
    private val sampleDataSeeder: com.forge.app.data.repo.SampleDataSeeder,
    private val adaptationRepo: com.forge.app.data.repo.AdaptationRepository,
    private val directiveRepo: com.forge.app.data.repo.DirectiveRepository,
    private val programRepo: com.forge.app.data.repo.ProgramRepository,
    private val programChangeGuard: com.forge.app.ui.common.ProgramChangeGuard,
    private val healthConnectManager: com.forge.app.data.health.HealthConnectManager,
    private val timeSignals: com.forge.app.core.time.TimeSignals,
    private val clock: Clock
) : ViewModel() {

    // ── design/surface-experiment (2026-08-15) ────────────────────────────────────────────────
    // The card-led Home opens with a header row (name · bell · Profile), so it needs the name
    // Profile already owns. Read here rather than shared, and dropped with the branch.
    //
    // It briefly carried the avatar too, for a photo in the header. That came out on 2026-08-15:
    // the stored image is a COVER, a background rather than a portrait, and a 36dp circle crop of
    // one claims a face the app has never asked for.

    /**
     * REMOVED with the rest of P-15: `weeklyVolumeSeries` and the unbounded
     * `observeAllFinishedSessions()` flow behind it filled two state fields no Home composable
     * reads. Keeping them "for the surface experiment" kept an O(N) pass over the entire session
     * history one combine line away from being live again, and kept the fields alive so nothing
     * flagged them as dead. Both are in the history if that branch returns.
     */

    /** Today's watch steps against a typical day (W6) — null hides the Home movement line entirely. */
    data class TodayMovement(val steps: Int, val typicalSteps: Int?)

    private val _movement = MutableStateFlow<TodayMovement?>(null)
    val movement: StateFlow<TodayMovement?> = _movement

    /**
     * Refresh the Home movement line (W6): today's Health Connect step total + the median of the
     * previous 14 full days as "typical" (null below 3 days of history — no fake baseline). Fail-soft:
     * not granted / no provider / read error → null → the line simply doesn't render (GYMAP-64 rule).
     */
    fun refreshMovement() = viewModelScope.launch {
        if (!healthConnectManager.canReadSteps()) { _movement.value = null; return@launch }
        val zone = java.time.ZoneId.systemDefault()
        val now = clock.nowMs()
        val today = java.time.Instant.ofEpochMilli(now).atZone(zone).toLocalDate()
        val todayStartMs = today.atStartOfDay(zone).toInstant().toEpochMilli()
        val sinceMs = today.minusDays(14).atStartOfDay(zone).toInstant().toEpochMilli()
        val days = healthConnectManager.readDailyStepTotals(sinceMs, now)
        val todaySteps = days.lastOrNull { it.dayStartMs == todayStartMs }?.steps ?: 0
        val prior = days.filter { it.dayStartMs < todayStartMs && it.steps > 0 }.map { it.steps }.sorted()
        val typical = if (prior.size >= 3) prior[prior.size / 2] else null
        _movement.value = TodayMovement(steps = todaySteps, typicalSteps = typical)
    }

    private val _coach = MutableStateFlow<List<CoachItem>>(emptyList())

    /** Sub-gate "still learning" nudge (CD-1) — null once the coach has activated. */
    private val _coachLearning = MutableStateFlow<CoachLearningHint?>(null)

    /** Sub-threshold fatigue nudge (Tier 3) — null unless the active coach is quiet but fatigue builds. */
    private val _coachFatigue = MutableStateFlow<com.forge.app.ui.overview.state.FatigueHint?>(null)

    /**
     * Today's directive (Coach v3 B2) — the hero's content. Loaded once per open and refreshed on
     * resume, like the movement line: the answer changes with the day, not with every emission.
     */
    private val _directive = MutableStateFlow<com.forge.app.data.repo.DirectiveRepository.TodayAnswer?>(null)

    /** "Go with the flow" — when on, the home leads with freestyle logging instead of a next-up day. */
    val freestyleMode: StateFlow<Boolean> =
        settingsRepo.freestyleMode.stateIn(viewModelScope, kotlinx.coroutines.flow.SharingStarted.WhileSubscribed(5_000), false)

    /** Whether the coach is surfaced on the home — off hides all coach banners/cards (declined in onboarding). */
    val coachEnabled: StateFlow<Boolean> =
        settingsRepo.coachEnabled.stateIn(viewModelScope, kotlinx.coroutines.flow.SharingStarted.WhileSubscribed(5_000), true)

    /** True when there is no program at all (build-your-own, not yet built) — home offers "build a plan".
     *  Seeds from the already-loaded facade so a no-plan user never flashes the "Start session" branch
     *  (with an empty day key) before the first revision tick arrives. */
    val programEmpty: StateFlow<Boolean> =
        programRepo.revision.map { com.forge.app.program.Program.days.isEmpty() }
            .stateIn(
                viewModelScope, kotlinx.coroutines.flow.SharingStarted.WhileSubscribed(5_000),
                com.forge.app.program.Program.days.isEmpty()
            )

    /** Goals for the Home preview, recomputed whenever a goal is added/edited/removed OR one of the
     *  progress INPUTS moves: a session finishes (lift bests + weekly tallies), cardio is logged or
     *  edited, or a new weigh-in lands. The input tables are observed directly rather than
     *  re-subscribing observeWeeklyStats() (which the main combine below already collects) — that
     *  double subscription recomputed goals on stats ticks yet still missed bodyweight logs, leaving
     *  a bodyweight goal line stale until an unrelated event. All reads are cheap aggregate queries. */
    private val goalsFlow: kotlinx.coroutines.flow.Flow<Pair<List<com.forge.app.data.repo.GoalRepository.GoalProgress>, List<com.forge.app.data.repo.ExtendedGoalRepository.Progress>>> =
        combine(
            goalRepo.observeAll(),
            extendedGoalRepo.observeAll(),
            sessionDao.observeFinishedCount(),
            cardioRepo.observeMinutesSince(0L),
            bodyweightRepo.observeRecent(1),
            // The calendar is an input too (M-32). A weekly goal's window is derived at read time,
            // so its progress only moved when a table did: leave Home open from Sunday into Monday
            // and a completed weekly goal kept reading 4 / 4 in a week where nothing had happened
            // yet. Monthly rollover and a timezone change behaved the same way.
            timeSignals.dayStarts()
            // Six flows selects combine's VARARG overload, whose transform takes a single
            // Array<Any?> — not six parameters. None of the emitted values is read (each is only a
            // "something moved, recompute" tick), so the array is discarded wholesale.
        ) { _ ->
            // Fall back on real failures only — a swallowed CancellationException would let a
            // cancelled recompute emit empty lists and blank the Home goal lines.
            val lift = runCatching { goalRepo.goalsWithProgress() }
                .getOrElse { if (it is kotlinx.coroutines.CancellationException) throw it else emptyList() }
            val custom = runCatching { extendedGoalRepo.goalsWithProgress() }
                .getOrElse { if (it is kotlinx.coroutines.CancellationException) throw it else emptyList() }
            lift to custom
        }.flowOn(Dispatchers.Default)

    // P-15: six inputs, each of which reaches something Home draws. Three more were subscribed for
    // state fields no composable read — the unlocked-trophy ID LIST (collected only to take its
    // size), the week's cardio distance, and the cardio weekly target — so every trophy unlock,
    // every cardio row and every settings change woke this combine and rebuilt the whole Home state
    // to fill fields that were then thrown away. The fields are gone with them; a surface that
    // wants them adds the input back beside the field it feeds.
    val state: StateFlow<OverviewUiState> = combine(
        statsRepo.observeWeeklyStats(),
        cardioRepo.observeRecent(7),
        settingsRepo.shownMilestones,
        statsRepo.observeDayVolumeStats(),
        settingsRepo.weightUnit,
        settingsRepo.useMiles
    ) { args ->
        val stats = args[0] as StatsRepository.WeeklyStats
        @Suppress("UNCHECKED_CAST")
        val recentCardio = args[1] as List<com.forge.app.data.db.entities.CardioEntry>
        @Suppress("UNCHECKED_CAST")
        val shown = args[2] as Set<String>
        @Suppress("UNCHECKED_CAST")
        val dayVolStats = args[3] as Map<String, SessionDao.DayVolumeStats>
        val weightUnit = args[4] as WeightUnit
        val useMiles = args[5] as Boolean

        buildOverviewUiState(
            stats = stats,
            recentCardio = recentCardio,
            shown = shown,
            dayVolStats = dayVolStats,
            weightUnit = weightUnit,
            useMiles = useMiles
        )
    }.combine(customizationRepo.observeAllDayNames()) { s, names ->
        val customName = names.firstOrNull { it.dayKey == s.nextUpDayKey }?.customName
        s.copy(customDayName = customName)
    }.combine(workoutRepo.observeActiveSession()) { s, active ->
        // Guard against a "zombie" active session whose day was removed by a program regenerate (or a
        // force-stop mid-first-gen): don't offer to resume a day that no longer exists — Program.day()
        // would silently resolve the stale key to the wrong day. An invalid key reads as no resume here.
        s.copy(activeSessionDayKey = active?.dayKey?.takeIf { it in com.forge.app.program.Program.dayKeys })
    }.combine(_directive) { s, answer ->
        s.copy(directive = answer?.directive, brief = answer?.brief, coldStartLesson = answer?.coldStartLesson)
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
        s.copy(weeklyTrainingDays = target)
    }.combine(freestyleMode) { s, freestyle ->
        // No fixed plan (freestyle) or no program yet → there's no weekly target to count toward, so
        // emit 0 (the home suppresses the "of N target" line when it's 0).
        if (freestyle || com.forge.app.program.Program.days.isEmpty()) s.copy(weeklyTrainingDays = 0) else s
    }.combine(settingsRepo.weightUnit) { s, weightUnit ->
        // Attach each recent gym row's marquee lift (its heaviest set). Each lookup is a real DB
        // read, so withTopLifts memoizes by session: a finished session's top set is immutable.
        // Without this the read re-ran on EVERY emission of the combine chain above (active-session,
        // coach, weekly-stats ticks) for the same unchanged sessions. weightUnit is part of the cache
        // signature so flipping the unit re-formats the marquee instead of serving a stale string.
        s.copy(recentItems = withTopLifts(s.recentItems, weightUnit))
    }.combine(goalsFlow) { s, gc ->
        s.copy(goals = gc.first, customGoals = gc.second)
    }.combine(timeSignals.dayStarts()) { s, todayStartMs ->
        // The day the state describes, so the goal captions below can be memoised against it
        // rather than against a goal object that may not change across a period boundary (M-32).
        s.copy(todayStartMs = todayStartMs)
    }.flowOn(Dispatchers.Default).stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = OverviewUiState()
    )

    init {
        // reloadCoach() is deliberately NOT run here (P-14). It fills three state flows — coach,
        // coachLearning, coachFatigue — that no composable on the shipped Home reads, and it is not
        // a cheap read: the adaptation snapshot behind it walks every finished session, its logged
        // exercises and its sets, then the sessions again through life events, plus check-ins,
        // cardio, restrictions, moods, bodyweight, swaps, preferences, cooldowns and a Health
        // Connect recovery read — on every Home open. The fields, the mapping and [refreshCoach]
        // stay, so a surface that brings the cards back asks for them.
        refreshDirective()
        refreshMovement()
        // Backfill the first-touch flag for users who already have history, so the onboarding cards
        // never reappear for a returning user (e.g. after a data wipe). finishWorkout() sets it going forward.
        viewModelScope.launch {
            if (statsRepo.observeWeeklyStats().first().totalFinishedSessions > 0) settingsRepo.setFirstWorkoutDone()
        }
        // Detect + resolve a zombie active session (force-stop / regenerate artifact) so it can't drive
        // a misleading resume or block a fresh start; surface what happened once (E8). Await ensureLoaded
        // FIRST: Program.dayKeys reports the hard-coded SEED split until the DB program is loaded, so
        // resolving against it on a cold start could finish a perfectly valid session whose day simply
        // isn't in the seed. ensureLoaded is idempotent + serialized, so this just waits out startup.
        //
        // The notice is WRITTEN rather than held in memory: it now surfaces in the notifications feed,
        // which has to be able to show it long after the open that resolved it.
        viewModelScope.launch {
            runCatching {
                programRepo.ensureLoaded()
                workoutRepo.resolveOrphanSession(com.forge.app.program.Program.dayKeys.toSet())
            }.getOrNull()?.let { res ->
                settingsRepo.addSystemNotice(
                    com.forge.app.data.repo.NotificationFeed.NOTICE_ORPHAN_SESSION,
                    if (res.finishedToHistory)
                        "Saved an unfinished workout from a day that's no longer in your program. It's in your history now."
                    else
                        "Cleared an empty leftover session from a workout day that's no longer in your program."
                )
            }
        }
    }

    /**
     * Recompute today's answer (Coach v3 B2). Called at open and on resume: a directive that still
     * says "Push day" after you've trained, or after midnight, is worse than no directive.
     */
    fun refreshDirective() = viewModelScope.launch {
        _directive.value = runCatching { directiveRepo.today() }.getOrNull()
    }

    /**
     * "Try demo data" opt-in (Cat 10): wire the otherwise-unreachable [SampleDataSeeder] to the
     * zero-session welcome card so a new user can populate Stats/rank/coach in one tap. Guarded on an
     * empty history so it can't double-seed; the Overview state refreshes itself via the DB flows.
     */
    fun loadSampleData() = viewModelScope.launch {
        if (sessionDao.finishedCount() == 0) sampleDataSeeder.seed()
    }

    // ─── Coach feed (adaptation engine) ───────────────────────────────────────

    private suspend fun reloadCoach() {
        val feed = adaptationRepo.coachFeed()
        val recs = feed.recommendations
            .mapNotNull { it.toCoachItem() }
            // Keep the engine's TOP 3 first (so a high-priority read-only nudge it ranked isn't bumped
            // out by lower-ranked actions), THEN float the actionable ones to the top within those 3.
            // Stable sort preserves the engine's order inside each group.
            .take(3)
            .sortedByDescending { it.applyLabel != null }
        _coach.value = recs
        // CD-1: only nudge "still learning" when there's nothing actionable AND the weekly pass
        // hasn't activated yet (below MIN_SESSIONS). Cheap finished-count query, off the hot path.
        // TRACKED sessions: this gates the coach, and an untracked session is excluded from
        // suggestions by contract. The seed gate above deliberately stays inclusive — that one asks
        // "does this install hold any history", which untracked history certainly is.
        val logged = sessionDao.trackedFinishedCount()
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
     * Recompute the Home coach cards. Not called at init (see there); the entry point exists so the
     * surface that renders [OverviewUiState.coach] again asks for them explicitly.
     */
    fun refreshCoach() = viewModelScope.launch { reloadCoach() }

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
    private val topLiftCache = ConcurrentHashMap<Long, Pair<Pair<Double?, WeightUnit>, String?>>()

    private suspend fun withTopLifts(items: List<OverviewRecentItem>, weightUnit: WeightUnit): List<OverviewRecentItem> =
        items.map { item ->
            if (!item.isGym || item.id < 0) return@map item
            val sig = item.volumeLb to weightUnit
            val cached = topLiftCache[item.id]
            val lift = if (cached != null && cached.first == sig) cached.second
                else topLiftFor(item.id, weightUnit).also { topLiftCache[item.id] = sig to it }
            item.copy(topLift = lift)
        }

    /** Heaviest weighted set of a session, formatted "Name 185 lb × 5". Null for bodyweight-only days. */
    private suspend fun topLiftFor(sessionId: Long, weightUnit: WeightUnit): String? {
        val top = statsRepo.getSessionExerciseLines(sessionId)
            .filter { (it.topWeightLb ?: 0.0) > 0.0 }
            .maxByOrNull { it.topWeightLb!! } ?: return null
        val wText = formatWeight(top.topWeightLb!!, weightUnit)
        return "${top.exerciseName} $wText × ${top.topReps ?: 0}"
    }
}
