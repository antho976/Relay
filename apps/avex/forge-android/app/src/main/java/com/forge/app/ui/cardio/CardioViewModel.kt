package com.forge.app.ui.cardio

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.core.time.Clock
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.repo.CardioRepository
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType
import com.forge.app.ui.cardio.state.CardioUiState
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
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
    private val clock: Clock
) : ViewModel() {

    private val transient = MutableStateFlow(TransientState())
    private val weekStartMs: Long = clock.nowMs() - WEEK_MS

    val state: StateFlow<CardioUiState> = combine(
        cardioRepo.observeRecent(limit = 20),
        cardioRepo.observeMinutesSince(weekStartMs),
        cardioRepo.observeSince(weekStartMs),
        transient
    ) { recent, weekMin, weekEntries, tr ->
        CardioUiState(
            isLoading = false,
            weekMinutes = weekMin ?: 0,
            weekEntryCount = weekEntries.count { it.type != CardioType.REST.code },
            weekDailyMinutes = buildDailyMinutes(weekEntries),
            entries = recent,
            sheetOpen = tr.sheetOpen,
            pendingDeleteId = tr.pendingDeleteId
        )
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
        initialValue = CardioUiState()
    )

    fun openSheet() = transient.update { it.copy(sheetOpen = true) }
    fun closeSheet() = transient.update { it.copy(sheetOpen = false) }

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
     * Persists a new entry. Caller has already validated duration > 0 (or that
     * this is a REST day where duration may be 0). Distance / effort / restReason
     * are nullable in the DB; pass null when the form chose to skip them.
     */
    fun logEntry(
        type: CardioType,
        durationMin: Int,
        distanceKm: Double?,
        effort: CardioEffort?,
        restReason: CardioRestReason?,
        note: String?
    ) {
        viewModelScope.launch {
            cardioRepo.add(
                CardioEntry(
                    date = clock.nowMs(),
                    type = type.code,
                    durationMin = durationMin.coerceAtLeast(0),
                    distanceKm = if (type.isRest) null else distanceKm,
                    effort = if (type.isRest) null else effort?.code,
                    restReason = if (type.isRest) restReason?.code else null,
                    note = note?.takeIf { it.isNotBlank() }
                )
            )
            transient.update { it.copy(sheetOpen = false) }
        }
    }

    private data class TransientState(
        val sheetOpen: Boolean = false,
        val pendingDeleteId: Long? = null
    )

    companion object {
        private const val WEEK_MS: Long = 7L * 24 * 60 * 60 * 1000

        /** Returns 7 minute totals [Mon, Tue, Wed, Thu, Fri, Sat, Sun] for the current week. */
        fun buildDailyMinutes(entries: List<CardioEntry>): List<Int> {
            val zone = ZoneId.systemDefault()
            val today = LocalDate.now(zone)
            val monday = today.with(DayOfWeek.MONDAY)
            return (0..6).map { dayOffset ->
                val day = monday.plusDays(dayOffset.toLong())
                if (day.isAfter(today)) {
                    0
                } else {
                    entries
                        .filter { e ->
                            e.type != CardioType.REST.code &&
                                Instant.ofEpochMilli(e.date).atZone(zone).toLocalDate() == day
                        }
                        .sumOf { it.durationMin }
                }
            }
        }
    }
}
