package com.forge.app.ui.cardio

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.core.time.Clock
import com.forge.app.data.db.entities.CardioEntry
import androidx.health.connect.client.records.ExerciseRoute
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.CardioRepository
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.TrophyRepository
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioCondition
import com.forge.app.domain.cardio.cardioActivityRecords
import com.forge.app.domain.cardio.CardioWeekAggregate
import com.forge.app.domain.cardio.cardioPaceSeries
import com.forge.app.domain.cardio.cardioWeekAggregate
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioField
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.CustomCardioType
import com.forge.app.domain.cardio.RoutePoint
import com.forge.app.data.health.HcExerciseTypes
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.health.HrPoint
import com.forge.app.domain.health.WatchWorkout
import com.forge.app.domain.health.downsampleHr
import com.forge.app.ui.cardio.state.CardioDayCell
import com.forge.app.ui.cardio.state.CardioLens
import com.forge.app.ui.cardio.state.CardioUiState
import com.forge.app.ui.common.SnackbarController
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.shareIn
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Phase 7 — cardio log. Three reactive inputs combined into one UI state:
 *   - recent entries (20 most recent)
 *   - "minutes this week" (excludes rest entries, per CardioDao default)
 *   - transient sheet/dialog state
 *
 * The weekly window follows the calendar: it is re-anchored on every local day boundary and on any
 * clock or timezone change, so a screen left open across a Sunday night shows the new week's dates
 * over the new week's numbers.
 */
@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
@HiltViewModel
class CardioViewModel @Inject constructor(
    private val cardioRepo: CardioRepository,
    private val settingsRepo: SettingsRepository,
    private val trophyRepo: TrophyRepository,
    private val extendedGoalRepo: ExtendedGoalRepository,
    private val healthConnectManager: HealthConnectManager,
    private val snackbar: SnackbarController,
    private val clock: Clock,
    private val timeSignals: com.forge.app.core.time.TimeSignals
) : ViewModel() {

    private val transient = MutableStateFlow(TransientState())

    /** The in-flight [saveEntry] write, if any — the double-tap guard. */
    private var saveJob: kotlinx.coroutines.Job? = null
    // Which Health Connect grants Avex actually holds — re-checked on init and on every resume (the
    // user may grant them in the HC app and return). Drives the banner's auto-hide and the "connected
    // but no data yet" steps placeholder. A one-shot read, not a flow: HC has no permission-change
    // observable, so we poll it at the moments it can change (resume) rather than continuously.
    private val connection = MutableStateFlow(WearableConnection())
    /**
     * Local midnight of TODAY — the anchor for everything on this screen that depends on the
     * calendar: the "this week" label, the Mon–Sun bars, the streak, the week's totals, and which
     * cell is today.
     *
     * A StateFlow, not a `val` captured at construction. A ViewModel outlives the screen it belongs
     * to, and a phone left on the Cardio tab over a Sunday night kept a Monday from the week before:
     * the label and the day cells are drawn from a FRESH `LocalDate.now()`, so the screen showed the
     * new week's dates over the old week's minutes, distance and streak — with no reload in sight,
     * because nothing in the pipeline had changed.
     *
     * The DAY, not the Monday it belongs to (M-15). The anchor used to be reduced to a week start
     * before anything downstream saw it, so a Tuesday-to-Sunday boundary was an equal value that a
     * [MutableStateFlow] drops and nothing re-ran at all.
     *
     * Driven by [com.forge.app.core.time.TimeSignals.dayStarts] rather than only by ON_RESUME.
     * Refreshing on resume fixes leaving and coming back, and does nothing at all for the screen
     * that is STILL OPEN when the date turns over — which is the case the bug was reported for:
     * nothing resumes, so nothing re-anchors, and the mismatch persists for as long as the phone
     * stays on the tab. `dayStarts()` emits immediately, at each local midnight, and on any clock or
     * timezone change, which is the same signal `StatsRepository` already uses for exactly this.
     */
    private val dayStart = MutableStateFlow(com.forge.app.core.time.dayStartMs(clock.nowMs()))

    init {
        refreshConnection()
        viewModelScope.launch {
            timeSignals.dayStarts().collect { start ->
                val moved = start != dayStart.value
                dayStart.value = start
                // Today's steps are a BOUNDED read of one calendar day, so a new day needs a new
                // one: it used to be taken on resume only, and a screen left open past midnight
                // kept yesterday's hourly bars and yesterday's total under the word "today" (M-15).
                if (moved) refreshTodaySteps()
            }
        }
    }

    /**
     * Re-anchor the day (and with it the week) if the calendar has moved on.
     *
     * The anchor is the local DAY, not the Monday it belongs to (M-15). Reducing the signal to a
     * Monday made every day-boundary from Tuesday to Sunday an equal value that a [MutableStateFlow]
     * drops, so nothing downstream re-ran: the Mon–Sun cells kept styling Monday as today, the
     * streak kept counting to yesterday, and the week's active-day count went stale — all on a
     * screen that was still open and had nothing else to change.
     *
     * Still called from ON_RESUME as well as from the day-boundary signal above: a device that
     * dozed through midnight can come back before the tick lands, and re-anchoring twice to the
     * same day costs nothing.
     */
    fun refreshWeekAnchor() {
        val current = com.forge.app.core.time.dayStartMs(clock.nowMs())
        if (current != dayStart.value) {
            dayStart.value = current
            refreshTodaySteps()
        }
    }

    /** Re-read whether the steps / exercise grants are held (call on resume — grants change in the HC app).
     *  Also refreshes today's step total for the hero line (GYMAP-64) so it tracks steps taken while away. */
    fun refreshConnection() = viewModelScope.launch {
        val steps = healthConnectManager.canReadSteps()
        val routes = healthConnectManager.canReadExercise()
        val hr = healthConnectManager.canReadHeartRate()
        // Today's steps, hourly bars included — one read feeds the whole STEPS section. Fail-soft to
        // null so a read error just hides it rather than drawing a broken mark.
        val today = if (steps) runCatching { loadStepsForDay(clock.nowMs()) }.getOrNull() else null
        connection.value = WearableConnection(steps = steps, routes = routes, today = today, hr = hr)
        // Candidate watch workouts for "recorded with your watch — import?" (W5). Already-logged and
        // dismissed sessions are filtered downstream against the live entry list.
        watchCandidates.value = if (routes) {
            val now = clock.nowMs()
            healthConnectManager.recentWatchWorkouts(now - IMPORT_LOOKBACK_MS, now, limit = 6)
        } else emptyList()
    }

    /**
     * Re-read TODAY's step total and hourly bars, without re-checking grants (M-15).
     *
     * The read is bounded to one calendar day, so it goes stale the moment the day turns over. It
     * was taken on resume only, which does nothing for the screen that is still open at midnight —
     * the case the finding is about. Fail-soft to leaving what is there rather than blanking the
     * section on a read error.
     */
    private fun refreshTodaySteps() = viewModelScope.launch {
        if (!connection.value.steps) return@launch
        val today = runCatching { loadStepsForDay(clock.nowMs()) }.getOrNull() ?: return@launch
        connection.update { it.copy(today = today) }
    }

    // Raw watch-session candidates from Health Connect, refreshed with the connection (init/resume).
    private val watchCandidates = MutableStateFlow<List<WatchWorkout>>(emptyList())

    // One shared subscription to the full history — both the derived aggregates and the cardio-goals
    // recompute read it, so a cardio write runs the whole-history query once, not once per consumer.
    private val entriesFlow = cardioRepo.observeAll()
        .shareIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), replay = 1)

    // Candidates minus sessions that already have a matching cardio entry, minus dismissed ones —
    // recomputed reactively when entries land or a suggestion is dismissed, capped for the section.
    private val importSuggestionsFlow = combine(
        watchCandidates, entriesFlow, settingsRepo.hcDismissedWatchImports
    ) { candidates, entries, dismissed ->
        candidates
            .filterNot { it.recordId in dismissed }
            .filterNot { w -> entries.any { e -> overlapsEntry(w, e) } }
            .filter { it.durationMin >= 1 }
            .take(3)
    }.flowOn(Dispatchers.Default)

    // All DB-derived aggregates (streak, weekly/last-week totals, the Mon–Sun cells) are computed
    // here off the DB flow and on Dispatchers.Default — NOT in the combine with `transient` below,
    // so toggling the sheet (a pure UI event) never re-runs the full-history streak/day passes, and
    // none of it runs on the main thread.
    private val derivedFlow = dayStart.flatMapLatest { todayStartMs ->
        entriesFlow.map { all -> all to todayStartMs }
    }.map { (all, todayStartMs) ->
        val weekStartMs = com.forge.app.core.time.mondayStartMs(todayStartMs)
        val zone = ZoneId.systemDefault()
        // Today comes from the ANCHOR, not from a fresh LocalDate.now() inside each helper: the
        // anchor is what the day-boundary tick moves, so deriving from it is what makes "today"
        // change on the screen that is still open, and what makes the helpers testable at all.
        val today = Instant.ofEpochMilli(todayStartMs).atZone(zone).toLocalDate()
        // The week's slice comes from the history ALREADY IN HAND, not from two more observers on
        // the same table (P-08). Combining `observeAll` with `observeMinutesSince` and
        // `observeSince` meant one insert invalidated three SQL queries, so a single logged session
        // could re-run the full-history streak, records and pace passes more than once — and, in
        // between, mix a new slice with an old whole. The filters below are exactly what those two
        // queries were: `date >= weekStart`, and the same sum over non-rest rows (a SUM over no
        // rows is NULL, which the caller read as 0).
        val weekEntries = all.filter { it.date >= weekStartMs }
        val weekMinutes = weekEntries
            .filter { it.type != CardioType.REST.code }
            .sumOf { it.durationMin }
        CardioDerived(
            all = all,
            todayStartMs = todayStartMs,
            weekMinutes = weekMinutes,
            cardioDaysThisWeek = countActiveDays(weekEntries, zone),
            cardioStreakDays = computeCardioStreak(all, zone, today),
            weekDays = buildWeekDays(weekEntries, zone, today),
            weekDistanceKm = weekEntries
                .filter { it.type != CardioType.REST.code }
                .sumOf { it.distanceKm ?: 0.0 },
            // All-time per-activity bests (GYMAP-34) — off the full history, on this same background pass.
            records = cardioActivityRecords(all),
            // Per-activity pace series (GYMAP-35) for the PROGRESS lens's trend chart.
            paceSeries = cardioPaceSeries(all),
            weekAggregate = cardioWeekAggregate(all, weekStartMs, zone)
        )
    }.flowOn(Dispatchers.Default)

    // The cardio-specific custom goals (distance / minutes), recomputed when a goal is added/edited
    // or a cardio entry lands — the same observe-inputs pattern as Home's goalsFlow, filtered to the
    // metrics this page owns. Home keeps the mixed top-3; this is the cardio lens on the same data.
    private val cardioGoalsFlow = combine(
        extendedGoalRepo.observeAll(),
        entriesFlow
    ) { _, _ ->
        // Fall back on real failures only — a swallowed CancellationException would let a cancelled
        // recompute emit an empty list and blank the goal lines.
        runCatching { extendedGoalRepo.goalsWithProgress() }
            .getOrElse { if (it is kotlinx.coroutines.CancellationException) throw it else emptyList() }
            .filter { it.metric == GoalMetric.CARDIO_DISTANCE || it.metric == GoalMetric.CARDIO_MINUTES }
    }.flowOn(Dispatchers.Default)

    val state: StateFlow<CardioUiState> = combine(
        derivedFlow, transient, settingsRepo.cardioWeeklyTargetMin, cardioGoalsFlow
    ) { d, tr, target, cardioGoals ->
        CardioUiState(
            isLoading = false,
            todayStartMs = d.todayStartMs,
            weekMinutes = d.weekMinutes,
            cardioDaysThisWeek = d.cardioDaysThisWeek,
            weekTargetMin = target,
            cardioStreakDays = d.cardioStreakDays,
            weekDays = d.weekDays,
            weekDistanceKm = d.weekDistanceKm,
            cardioRecords = d.records,
            cardioPaceSeries = d.paceSeries,
            weekAggregate = d.weekAggregate,
            cardioGoals = cardioGoals,
            lens = tr.lens,
            entries = d.all,
            sheetOpen = tr.sheetOpen,
            editing = tr.editing,
            sessionDetailId = tr.sessionDetailId,
            sessionWearable = tr.sessionWearable,
            sessionRoute = tr.sessionRoute,
            sessionRouteConsentId = tr.sessionRouteConsentId,
            sessionHr = tr.sessionHr,
            sessionWatch = tr.sessionWatch,
            historyExpanded = tr.historyExpanded
        )
    }.combine(settingsRepo.useMiles) { st, useMiles ->
        st.copy(useMiles = useMiles)
    }.combine(connection) { st, conn ->
        st.copy(
            stepsConnected = conn.steps, routesConnected = conn.routes,
            todayWearable = conn.today, hrConnected = conn.hr
        )
    }.combine(settingsRepo.lastCardioType) { st, lastType ->
        st.copy(lastCardioType = lastType)
    }.combine(importSuggestionsFlow) { st, suggestions ->
        st.copy(importSuggestions = suggestions)
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = CardioUiState()
    )

    /** Open the sheet to log a NEW entry. */
    fun openSheet() = transient.update { it.copy(sheetOpen = true, editing = null) }

    /** Open the sheet pre-filled to edit an existing entry. */
    fun editEntry(id: Long) = viewModelScope.launch {
        val entry = cardioRepo.get(id) ?: return@launch
        transient.update { it.copy(sheetOpen = true, editing = entry) }
    }

    fun closeSheet() = transient.update { it.copy(sheetOpen = false, editing = null) }

    /** Switch the overview's lens (§4.4). Transient — leaving the tab returns to WEEK. */
    fun setLens(lens: CardioLens) = transient.update { it.copy(lens = lens) }

    /** Open / close the per-session stats overlay for a logged entry. Loads that day's watch steps and
     *  the best-matching GPS route in the background; each load is dropped if the user has since closed
     *  or switched the overlay. */
    fun openSessionDetail(id: Long) {
        transient.update {
            it.copy(
                sessionDetailId = id, sessionWearable = null, sessionRoute = null,
                sessionRouteConsentId = null, sessionHr = null, sessionWatch = null
            )
        }
        viewModelScope.launch {
            val entry = cardioRepo.get(id) ?: return@launch
            val zone = ZoneId.systemDefault()
            val day = Instant.ofEpochMilli(entry.date).atZone(zone).toLocalDate()
            val startMs = day.atStartOfDay(zone).toInstant().toEpochMilli()
            val endMs = day.plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli()
            val steps = healthConnectManager.readStepsDay(startMs, endMs)
            // A matched route either arrives ready to draw, or as a consent id the UI must confirm first.
            val match = healthConnectManager.matchSessionRoute(entry.date, entry.durationMin, startMs, endMs)
            // The same time-window match, for the watch's measured stats + HR series (W5) — each
            // independently fail-soft, so a missing grant just hides its section.
            val watch = healthConnectManager.matchWatchSession(entry.date, entry.durationMin, startMs, endMs)
            val hr = watch?.let {
                downsampleHr(healthConnectManager.readHrSeries(it.startMs, it.endMs)).takeIf { s -> s.size >= 2 }
            }
            transient.update {
                if (it.sessionDetailId == id) it.copy(
                    sessionWearable = steps,
                    sessionRoute = match?.route,
                    sessionRouteConsentId = if (match?.route == null) match?.recordId else null,
                    sessionHr = hr,
                    sessionWatch = watch
                ) else it
            }
        }
    }
    fun closeSessionDetail() = transient.update {
        it.copy(
            sessionDetailId = null, sessionWearable = null, sessionRoute = null,
            sessionRouteConsentId = null, sessionHr = null, sessionWatch = null
        )
    }

    /**
     * Adopt the matched watch session's measured duration/distance onto the open entry (W5) —
     * explicit and undoable, never a silent overwrite. Only fields the watch actually measured
     * change; everything the user typed (effort, note, conditions) stays.
     */
    fun adoptWatchStats() = viewModelScope.launch {
        val id = transient.value.sessionDetailId ?: return@launch
        val watch = transient.value.sessionWatch ?: return@launch
        val original = cardioRepo.get(id) ?: return@launch
        val updated = original.copy(
            durationMin = watch.durationMin.takeIf { it > 0 } ?: original.durationMin,
            distanceKm = watch.distanceKm ?: original.distanceKm
        )
        if (updated == original) return@launch
        cardioRepo.update(updated)
        snackbar.showUndo("Watch stats applied") { cardioRepo.update(original) }
    }

    /** Open the log sheet prefilled from a watch workout (W5 import). Saving inserts a NEW entry. */
    fun importWatchWorkout(w: WatchWorkout) = transient.update {
        it.copy(
            sheetOpen = true,
            editing = CardioEntry(
                id = 0,
                date = w.startMs,
                type = HcExerciseTypes.toCardioCode(w.exerciseType),
                durationMin = w.durationMin,
                distanceKm = w.distanceKm
            )
        )
    }

    /** Hide the current import suggestions (whole-section dismiss; they stay hidden for good). */
    fun dismissWatchImports() = viewModelScope.launch {
        val ids = state.value.importSuggestions.map { it.recordId }.toSet()
        if (ids.isNotEmpty()) settingsRepo.addDismissedWatchImports(ids)
    }

    /** Health Connect returned (or denied) the route after the consent screen. A non-null route with
     *  ≥2 points is drawn; anything less clears the pending consent so the button stops offering it. */
    fun onRouteConsented(route: ExerciseRoute?) {
        val points = healthConnectManager.routePoints(route).takeIf { it.size >= 2 }
        transient.update { it.copy(sessionRoute = points, sessionRouteConsentId = null) }
    }

    /** Read the watch's hourly steps for the local calendar day containing [dateMs] (empty when no
     *  wearable is connected or HC has nothing — the caller treats empty as "no graph"). */
    private suspend fun loadStepsForDay(dateMs: Long): CardioWearableDay {
        val zone = ZoneId.systemDefault()
        val day = Instant.ofEpochMilli(dateMs).atZone(zone).toLocalDate()
        val startMs = day.atStartOfDay(zone).toInstant().toEpochMilli()
        val endMs = day.plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli()
        return healthConnectManager.readStepsDay(startMs, endMs)
    }

    /** Reveal the full history below the 5 most-recent entries on the main list (or collapse it). */
    fun toggleHistoryExpanded() = transient.update { it.copy(historyExpanded = !it.historyExpanded) }

    /** Delete an entry now and offer an Undo (§13 undo over confirm) — the captured row re-inserts
     *  with its original id, so an undo restores it exactly. The row's reactive disappearance from the
     *  list (and the auto-close of an open session overlay) is the confirmation; no dialog. */
    fun deleteEntry(id: Long) = viewModelScope.launch {
        val entry = cardioRepo.get(id) ?: return@launch
        cardioRepo.delete(entry)
        snackbar.showUndo("Entry deleted") { cardioRepo.add(entry) }
    }

    /**
     * Persists the sheet — inserts a new entry, or updates the one being edited (keeping its id).
     * [dateMs] is the chosen day (backdating supported); the form validates duration/rest reason.
     * Distance / effort / restReason are nullable; pass null when the form skipped them.
     */
    fun saveEntry(
        activity: CardioActivity,
        durationMin: Int,
        distanceKm: Double?,
        effort: CardioEffort?,
        restReason: CardioRestReason?,
        note: String?,
        dateMs: Long,
        intervalCount: Int?,
        hrZone: String?,
        inclinePct: Double?,
        laps: Int?,
        elevationM: Double?,
        conditions: Set<CardioCondition>
    ) {
        // id 0 = a PREFILL (watch import, W5): the sheet showed it via `editing`, but saving inserts.
        val editingId = transient.value.editing?.id?.takeIf { it != 0L }
        // One save at a time, mirroring FreestyleLogViewModel's saveJob. Save stays ENABLED through
        // the write and the sheet only closes at the end of it, so a firm double tap launched two
        // coroutines that each read `editing` as null and each INSERTED — two identical sessions in
        // the history, and two toward every cardio goal and trophy counting them.
        if (saveJob?.isActive == true) return
        saveJob = viewModelScope.launch {
            val entry = CardioEntry(
                id = editingId ?: 0,
                date = dateMs,
                type = activity.code,
                durationMin = durationMin.coerceAtLeast(0),
                distanceKm = if (activity.isRest) null else distanceKm,
                effort = if (activity.isRest) null else effort?.code,
                restReason = if (activity.isRest) restReason?.code else null,
                note = note?.takeIf { it.isNotBlank() },
                // Interval count only applies to HIIT; HR zone to any active session. Cleared for rest.
                intervalCount = if (activity.isHiit) intervalCount?.takeIf { it > 0 } else null,
                hrZone = if (activity.isRest) null else hrZone,
                // Per-type fields (GYMAP-38): kept only for the activities that surface them, so a
                // value typed then switched away from (stale form state) is never persisted.
                inclinePct = inclinePct.takeIf { CardioField.INCLINE in activity.optionalFields && (it ?: 0.0) > 0.0 },
                laps = laps.takeIf { CardioField.LAPS in activity.optionalFields && (it ?: 0) > 0 },
                elevationM = elevationM.takeIf { CardioField.ELEVATION in activity.optionalFields && (it ?: 0.0) > 0.0 },
                // Weather tags (GYMAP-39) — descriptive only, and never on a rest day.
                conditions = if (activity.isRest) null else CardioCondition.encode(conditions)
            )
            if (editingId != null) cardioRepo.update(entry) else cardioRepo.add(entry)
            // Remember the activity as the next new-entry default (GYMAP-40) — only when logging a NEW
            // active session, so editing an old row or saving a rest day never changes the default.
            if (editingId == null && !activity.isRest) settingsRepo.setLastCardioType(activity.code)
            transient.update { it.copy(sheetOpen = false, editing = null) }
            // Cardio trophies (first run / 100 km / N sessions) evaluate here too, since cardio logging
            // doesn't go through the workout-finish path that normally unlocks trophies — but off the
            // critical path (it's a ~14-query snapshot) so the sheet closes immediately, not after it.
            viewModelScope.launch { runCatching { trophyRepo.evaluateAndUnlockNew() } }
        }
    }

    /** Persist a custom activity created inline from the log sheet (GYMAP-37). The picker + rows pick
     *  it up reactively via [LocalCardioTypes]; the sheet also selects it optimistically. */
    fun addCustomType(type: CustomCardioType) = viewModelScope.launch {
        settingsRepo.addCustomCardioType(type)
    }

    /** The Health Connect grants Avex holds for the cardio screen's wearable data. */
    private data class WearableConnection(
        val steps: Boolean = false,
        val routes: Boolean = false,
        /** Today's watch steps with their hourly split — the WEEK lens's STEPS mark. */
        val today: CardioWearableDay? = null,
        /** HeartRateRecord read granted (W5) — drives the session HR graph. */
        val hr: Boolean = false
    )

    /** A watch workout already has a home when a NON-REST entry overlaps its span (or starts within
     *  the slack of it) — those never surface as import suggestions. Pure; slack absorbs watch-vs-
     *  manual start-time fuzz. */
    private fun overlapsEntry(w: WatchWorkout, e: CardioEntry): Boolean {
        if (e.type == CardioType.REST.code) return false
        val entryStart = e.date
        val entryEnd = e.date + e.durationMin.coerceAtLeast(0) * 60_000L
        return entryStart < w.endMs + IMPORT_OVERLAP_SLACK_MS && w.startMs < entryEnd + IMPORT_OVERLAP_SLACK_MS
    }

    private data class TransientState(
        val sheetOpen: Boolean = false,
        val editing: CardioEntry? = null,
        val lens: CardioLens = CardioLens.WEEK,
        val sessionDetailId: Long? = null,
        val sessionWearable: CardioWearableDay? = null,
        val sessionRoute: List<RoutePoint>? = null,
        val sessionRouteConsentId: String? = null,
        /** Downsampled HR series of the open session's matched watch workout (W5); null when none. */
        val sessionHr: List<HrPoint>? = null,
        /** The open session's matched watch workout with its measured stats (W5); null when none. */
        val sessionWatch: WatchWorkout? = null,
        val historyExpanded: Boolean = false
    )

    /** DB-derived aggregates, computed off the DB flow on a background dispatcher. */
    private data class CardioDerived(
        val todayStartMs: Long,
        val all: List<CardioEntry>,
        val weekMinutes: Int,
        val cardioDaysThisWeek: Int,
        val cardioStreakDays: Int,
        val weekDays: List<CardioDayCell>,
        val weekDistanceKm: Double,
        val records: List<com.forge.app.domain.cardio.CardioActivityRecord>,
        val paceSeries: List<com.forge.app.domain.cardio.CardioPaceSeries>,
        val weekAggregate: CardioWeekAggregate
    )

    companion object {
        /** Mon–Sun cells for the current week: active minutes + whether a rest day was logged.
         *  One pass over the entries per day; future days are empty. */
        fun buildWeekDays(
            entries: List<CardioEntry>,
            zone: ZoneId = ZoneId.systemDefault(),
            today: LocalDate = LocalDate.now(zone)
        ): List<CardioDayCell> {
            val monday = today.with(DayOfWeek.MONDAY)
            return (0..6).map { dayOffset ->
                val day = monday.plusDays(dayOffset.toLong())
                if (day.isAfter(today)) {
                    CardioDayCell()
                } else {
                    val dayEntries = entries.filter {
                        Instant.ofEpochMilli(it.date).atZone(zone).toLocalDate() == day
                    }
                    val minutes = dayEntries.filter { it.type != CardioType.REST.code }.sumOf { it.durationMin }
                    // A rest day reads distinctly only when nothing active was also logged that day.
                    val isRest = minutes == 0 && dayEntries.any { it.type == CardioType.REST.code }
                    CardioDayCell(minutes = minutes, isRest = isRest)
                }
            }
        }

        /** How far back the watch-workout import suggestions look (W5). */
        private const val IMPORT_LOOKBACK_MS = 14L * 24 * 60 * 60 * 1000
        /** Start/overlap slack when deciding a watch workout already has a cardio entry (W5). */
        private const val IMPORT_OVERLAP_SLACK_MS = 45L * 60 * 1000

        /** Distinct calendar days among [weekEntries] that carry an active (non-rest) session.
         *  Two runs on the same day count once; rest-only days don't count. Drives the hero headline. */
        fun countActiveDays(weekEntries: List<CardioEntry>, zone: ZoneId): Int =
            weekEntries
                .asSequence()
                .filter { it.type != CardioType.REST.code }
                .mapTo(mutableSetOf()) { Instant.ofEpochMilli(it.date).atZone(zone).toLocalDate() }
                .size

        /** Consecutive calendar days, ending today or yesterday, with an active (non-rest) session. */
        fun computeCardioStreak(
            entries: List<CardioEntry>,
            zone: ZoneId,
            today: LocalDate = LocalDate.now(zone)
        ): Int {
            val days = entries
                .filter { it.type != CardioType.REST.code }
                .mapTo(sortedSetOf()) { Instant.ofEpochMilli(it.date).atZone(zone).toLocalDate() }
            if (days.isEmpty()) return 0
            var cursor = when {
                today in days -> today
                today.minusDays(1) in days -> today.minusDays(1)
                else -> return 0
            }
            var count = 0
            while (cursor in days) { count++; cursor = cursor.minusDays(1) }
            return count
        }
    }
}
