package com.forge.app.ui.cardio

import androidx.health.connect.client.records.ExerciseRoute
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.CardioRepository
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.RoutePoint
import com.forge.app.ui.nav.Routes
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.Instant
import java.time.ZoneId
import javax.inject.Inject

/** State for the routed, single-session cardio stats screen (reached from the History page). */
data class CardioSessionDetailState(
    val loaded: Boolean = false,
    val entry: CardioEntry? = null,
    /** Every logged entry — the compare pool for the detail sheet's best/previous reads. */
    val allEntries: List<CardioEntry> = emptyList(),
    val editing: Boolean = false,
    val deleted: Boolean = false,
    val useMiles: Boolean = false,
    /** Watch steps for the entry's day (null until loaded / when none). */
    val wearable: CardioWearableDay? = null,
    /** GPS route for the session once available (already-consented or just confirmed); else null. */
    val route: List<RoutePoint>? = null,
    /** A matching watch session has a route needing Health Connect consent; this is its record id. */
    val routeConsentId: String? = null,
    /** Avex holds the steps grant — show the steps section (with a placeholder) even before data syncs. */
    val stepsConnected: Boolean = false
)

/**
 * Backs the routed [CardioSessionDetailScreen]. Loads a single [CardioEntry] by id (reactively, so an
 * edit refreshes the view), and offers the same edit/delete actions the in-screen cardio overlay has —
 * so opening a cardio session from History behaves identically to opening it from the cardio tab.
 */
@HiltViewModel
class CardioSessionDetailViewModel @Inject constructor(
    private val cardioRepo: CardioRepository,
    settingsRepo: SettingsRepository,
    private val healthConnectManager: HealthConnectManager,
    savedStateHandle: SavedStateHandle
) : ViewModel() {

    private val cardioId: Long = savedStateHandle.get<Long>(Routes.ARG_CARDIO_ID) ?: -1L

    private val editing = MutableStateFlow(false)
    private val deleted = MutableStateFlow(false)
    // Watch steps + GPS route + grant — loaded fail-soft for the entry's day, so opening a session from
    // History shows the same wearable detail (and the same connected-but-empty placeholder) as the tab.
    private val wearable = MutableStateFlow(WearableBits())
    // Tracks the in-flight wearable load so a new one cancels it first — otherwise a slower stale load
    // (an old entry-day, or a pre-grant canReadSteps=false snapshot) could finish last and overwrite the
    // newer result. loadWearable fires from both the entry-change collector and every ON_RESUME.
    private var wearableJob: Job? = null

    private val entryFlow = cardioRepo.observeAll().map { list -> list.firstOrNull { it.id == cardioId } }

    val state: StateFlow<CardioSessionDetailState> = combine(
        cardioRepo.observeAll(),
        editing,
        deleted,
        settingsRepo.useMiles
    ) { all, isEditing, isDeleted, useMiles ->
        CardioSessionDetailState(
            loaded = true,
            entry = all.firstOrNull { it.id == cardioId },
            allEntries = all,
            editing = isEditing,
            deleted = isDeleted,
            useMiles = useMiles
        )
    }.combine(wearable) { st, w ->
        st.copy(wearable = w.day, route = w.route, routeConsentId = w.routeConsentId, stepsConnected = w.connected)
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), CardioSessionDetailState())

    init {
        // Reload the wearable bits whenever the entry's day/length changes (an edit can reschedule it).
        viewModelScope.launch {
            entryFlow
                .map { it?.date to (it?.durationMin ?: 0) }
                .distinctUntilChanged()
                .collect { loadWearable() }
        }
    }

    /** Re-read the steps grant + the entry-day steps/route. Called on entry change and on resume.
     *  Fetches the entry fresh (not from [state]) so an edit that reschedules the day can't load stale.
     *  Cancels any in-flight load first so only the most-recent call writes [wearable]. */
    fun loadWearable() {
        wearableJob?.cancel()
        wearableJob = viewModelScope.launch {
            val entry = cardioRepo.get(cardioId) ?: run {
                wearable.value = WearableBits(connected = healthConnectManager.canReadSteps())
                return@launch
            }
            val zone = ZoneId.systemDefault()
            val day = Instant.ofEpochMilli(entry.date).atZone(zone).toLocalDate()
            val startMs = day.atStartOfDay(zone).toInstant().toEpochMilli()
            val endMs = day.plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli()
            val steps = healthConnectManager.readStepsDay(startMs, endMs)
            val match = healthConnectManager.matchSessionRoute(entry.date, entry.durationMin, startMs, endMs)
            wearable.value = WearableBits(
                day = steps,
                route = match?.route,
                routeConsentId = if (match?.route == null) match?.recordId else null,
                connected = healthConnectManager.canReadSteps()
            )
        }
    }

    /** Health Connect returned (or denied) the route after the consent screen — draw a ≥2-point track. */
    fun onRouteConsented(route: ExerciseRoute?) {
        val points = healthConnectManager.routePoints(route).takeIf { it.size >= 2 }
        wearable.update { it.copy(route = points, routeConsentId = null) }
    }

    fun openEdit() { editing.value = true }
    fun closeEdit() { editing.value = false }

    fun delete() {
        val entry = state.value.entry ?: return
        viewModelScope.launch {
            cardioRepo.delete(entry)
            deleted.value = true
        }
    }

    fun save(
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
        val current = state.value.entry ?: return
        viewModelScope.launch {
            cardioRepo.update(
                CardioEntry(
                    id = current.id,
                    date = dateMs,
                    type = type.code,
                    durationMin = durationMin.coerceAtLeast(0),
                    distanceKm = if (type.isRest) null else distanceKm,
                    effort = if (type.isRest) null else effort?.code,
                    restReason = if (type.isRest) restReason?.code else null,
                    note = note?.takeIf { it.isNotBlank() },
                    intervalCount = if (type == CardioType.HIIT) intervalCount?.takeIf { it > 0 } else null,
                    hrZone = if (type.isRest) null else hrZone
                )
            )
            editing.value = false
        }
    }

    /** The watch bits loaded for the entry's day — steps, GPS route (or its pending consent id), grant. */
    private data class WearableBits(
        val day: CardioWearableDay? = null,
        val route: List<RoutePoint>? = null,
        val routeConsentId: String? = null,
        val connected: Boolean = false
    )
}
