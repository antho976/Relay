package com.forge.app.ui.cardio

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.core.time.Clock
import com.forge.app.data.db.entities.CardioEntry
import androidx.health.connect.client.records.ExerciseRoute
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.BodyweightRepository
import com.forge.app.data.repo.CardioRepository
import com.forge.app.data.repo.TrophyRepository
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.RoutePoint
import com.forge.app.ui.cardio.state.CardioDayCell
import com.forge.app.ui.cardio.state.CardioUiState
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
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
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
 * Weekly-window start is captured once at construction. Matches the StatsRepository
 * behaviour: if the user keeps the app open for a week the window won't slide, but
 * that's fine for a personal app.
 */
@HiltViewModel
class CardioViewModel @Inject constructor(
    private val cardioRepo: CardioRepository,
    private val settingsRepo: SettingsRepository,
    private val trophyRepo: TrophyRepository,
    private val bodyweightRepo: BodyweightRepository,
    private val healthConnectManager: HealthConnectManager,
    private val clock: Clock
) : ViewModel() {

    private val transient = MutableStateFlow(TransientState())
    // Start of the current ISO week (Monday) — matches the "this week" label and the Mon–Sun bars
    // (was a rolling now-minus-7-days window). Captured once at construction.
    private val weekStartMs: Long = com.forge.app.core.time.mondayStartMs(clock.nowMs())

    // All DB-derived aggregates (streak, weekly/last-week totals, the Mon–Sun cells) are computed
    // here off the DB flow and on Dispatchers.Default — NOT in the combine with `transient` below,
    // so toggling the sheet (a pure UI event) never re-runs the full-history streak/day passes, and
    // none of it runs on the main thread.
    private val derivedFlow = combine(
        cardioRepo.observeAll(),
        cardioRepo.observeMinutesSince(weekStartMs),
        cardioRepo.observeSince(weekStartMs)
    ) { all, weekMin, weekEntries ->
        val zone = ZoneId.systemDefault()
        CardioDerived(
            all = all,
            weekMinutes = weekMin ?: 0,
            cardioDaysThisWeek = countActiveDays(weekEntries, zone),
            cardioStreakDays = computeCardioStreak(all, zone),
            weekDays = buildWeekDays(weekEntries)
        )
    }.flowOn(Dispatchers.Default)

    // Latest logged bodyweight, reactive — scales the calorie estimate on the log sheet + history rows.
    private val bodyweightLbFlow = bodyweightRepo.observeRecent(1).map { it.firstOrNull()?.weightLb }

    val state: StateFlow<CardioUiState> = combine(
        derivedFlow, transient, settingsRepo.cardioWeeklyTargetMin, bodyweightLbFlow,
        settingsRepo.cardioWearableHintDismissed
    ) { d, tr, target, bodyweightLb, hintDismissed ->
        CardioUiState(
            isLoading = false,
            weekMinutes = d.weekMinutes,
            cardioDaysThisWeek = d.cardioDaysThisWeek,
            weekTargetMin = target,
            cardioStreakDays = d.cardioStreakDays,
            weekDays = d.weekDays,
            entries = d.all,
            bodyweightLb = bodyweightLb,
            sheetOpen = tr.sheetOpen,
            editing = tr.editing,
            pendingDeleteId = tr.pendingDeleteId,
            detailOpen = tr.detailOpen,
            sessionDetailId = tr.sessionDetailId,
            sessionWearable = tr.sessionWearable,
            sessionRoute = tr.sessionRoute,
            sessionRouteConsentId = tr.sessionRouteConsentId,
            weekWearable = tr.weekWearable,
            historyExpanded = tr.historyExpanded,
            wearableHintDismissed = hintDismissed
        )
    }.combine(settingsRepo.useMiles) { st, useMiles ->
        st.copy(useMiles = useMiles)
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

    /** Open / close the swipeable week-stats overlay. The current-week page shows today's watch steps,
     *  loaded fail-soft (null when nothing's connected → the steps graph simply doesn't render). */
    fun openDetail() {
        transient.update { it.copy(detailOpen = true) }
        viewModelScope.launch {
            val today = loadStepsForDay(clock.nowMs())
            transient.update { if (it.detailOpen) it.copy(weekWearable = today) else it }
        }
    }
    fun closeDetail() = transient.update { it.copy(detailOpen = false, weekWearable = null) }

    /** Open / close the per-session stats overlay for a logged entry. Loads that day's watch steps and
     *  the best-matching GPS route in the background; each load is dropped if the user has since closed
     *  or switched the overlay. */
    fun openSessionDetail(id: Long) {
        transient.update {
            it.copy(sessionDetailId = id, sessionWearable = null, sessionRoute = null, sessionRouteConsentId = null)
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
            transient.update {
                if (it.sessionDetailId == id) it.copy(
                    sessionWearable = steps,
                    sessionRoute = match?.route,
                    sessionRouteConsentId = if (match?.route == null) match?.recordId else null
                ) else it
            }
        }
    }
    fun closeSessionDetail() = transient.update {
        it.copy(sessionDetailId = null, sessionWearable = null, sessionRoute = null, sessionRouteConsentId = null)
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

    /** Permanently dismiss the "connect a watch/ring" hint banner. */
    fun dismissWearableHint() = viewModelScope.launch { settingsRepo.setCardioWearableHintDismissed() }

    /** Reveal the full history below the 5 most-recent entries on the main list (or collapse it). */
    fun toggleHistoryExpanded() = transient.update { it.copy(historyExpanded = !it.historyExpanded) }

    fun requestDelete(id: Long) = transient.update { it.copy(pendingDeleteId = id) }
    fun cancelDelete() = transient.update { it.copy(pendingDeleteId = null) }

    fun confirmDelete() {
        val id = transient.value.pendingDeleteId ?: return
        viewModelScope.launch {
            val entry = cardioRepo.get(id) ?: return@launch
            cardioRepo.delete(entry)
            transient.update { it.copy(pendingDeleteId = null) }
        }
    }

    /**
     * Persists the sheet — inserts a new entry, or updates the one being edited (keeping its id).
     * [dateMs] is the chosen day (backdating supported); the form validates duration/rest reason.
     * Distance / effort / restReason are nullable; pass null when the form skipped them.
     */
    fun saveEntry(
        type: CardioType,
        durationMin: Int,
        distanceKm: Double?,
        effort: CardioEffort?,
        restReason: CardioRestReason?,
        note: String?,
        dateMs: Long,
        intervalCount: Int?,
        hrZone: String?
    ) {
        val editingId = transient.value.editing?.id
        viewModelScope.launch {
            val entry = CardioEntry(
                id = editingId ?: 0,
                date = dateMs,
                type = type.code,
                durationMin = durationMin.coerceAtLeast(0),
                distanceKm = if (type.isRest) null else distanceKm,
                effort = if (type.isRest) null else effort?.code,
                restReason = if (type.isRest) restReason?.code else null,
                note = note?.takeIf { it.isNotBlank() },
                // Interval count only applies to HIIT; HR zone to any active session. Cleared for rest.
                intervalCount = if (type == CardioType.HIIT) intervalCount?.takeIf { it > 0 } else null,
                hrZone = if (type.isRest) null else hrZone
            )
            if (editingId != null) cardioRepo.update(entry) else cardioRepo.add(entry)
            transient.update { it.copy(sheetOpen = false, editing = null) }
            // Cardio trophies (first run / 100 km / N sessions) evaluate here too, since cardio logging
            // doesn't go through the workout-finish path that normally unlocks trophies — but off the
            // critical path (it's a ~14-query snapshot) so the sheet closes immediately, not after it.
            viewModelScope.launch { runCatching { trophyRepo.evaluateAndUnlockNew() } }
        }
    }

    private data class TransientState(
        val sheetOpen: Boolean = false,
        val editing: CardioEntry? = null,
        val pendingDeleteId: Long? = null,
        val detailOpen: Boolean = false,
        val sessionDetailId: Long? = null,
        val sessionWearable: CardioWearableDay? = null,
        val sessionRoute: List<RoutePoint>? = null,
        val sessionRouteConsentId: String? = null,
        val weekWearable: CardioWearableDay? = null,
        val historyExpanded: Boolean = false
    )

    /** DB-derived aggregates, computed off the DB flow on a background dispatcher. */
    private data class CardioDerived(
        val all: List<CardioEntry>,
        val weekMinutes: Int,
        val cardioDaysThisWeek: Int,
        val cardioStreakDays: Int,
        val weekDays: List<CardioDayCell>
    )

    companion object {
        /** Mon–Sun cells for the current week: active minutes + whether a rest day was logged.
         *  One pass over the entries per day; future days are empty. */
        fun buildWeekDays(entries: List<CardioEntry>): List<CardioDayCell> {
            val zone = ZoneId.systemDefault()
            val today = LocalDate.now(zone)
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

        /** Distinct calendar days among [weekEntries] that carry an active (non-rest) session.
         *  Two runs on the same day count once; rest-only days don't count. Drives the hero headline. */
        fun countActiveDays(weekEntries: List<CardioEntry>, zone: ZoneId): Int =
            weekEntries
                .asSequence()
                .filter { it.type != CardioType.REST.code }
                .mapTo(mutableSetOf()) { Instant.ofEpochMilli(it.date).atZone(zone).toLocalDate() }
                .size

        /** Consecutive calendar days, ending today or yesterday, with an active (non-rest) session. */
        fun computeCardioStreak(entries: List<CardioEntry>, zone: ZoneId): Int {
            val days = entries
                .filter { it.type != CardioType.REST.code }
                .mapTo(sortedSetOf()) { Instant.ofEpochMilli(it.date).atZone(zone).toLocalDate() }
            if (days.isEmpty()) return 0
            val today = LocalDate.now(zone)
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
