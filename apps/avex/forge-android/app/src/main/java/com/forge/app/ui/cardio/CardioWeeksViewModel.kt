package com.forge.app.ui.cardio

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.core.time.Clock
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.CardioRepository
import com.forge.app.domain.cardio.CardioWeekPoint
import com.forge.app.domain.cardio.cardioWeekSeries
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import java.time.Instant
import java.time.ZoneId
import java.time.temporal.ChronoUnit
import javax.inject.Inject

/** State for the weeks ledger — every trained week as a row, newest first. */
data class CardioWeeksState(
    val loaded: Boolean = false,
    /** Newest week first — the ledger's reading order. Empty weeks between trained ones are kept. */
    val weeks: List<CardioWeekPoint> = emptyList(),
    /** Every entry, so an opened week can list its own sessions with no second query. */
    val entries: List<CardioEntry> = emptyList(),
    val weekTargetMin: Int = 0,
    val useMiles: Boolean = false,
    /** Non-null → the week detail is open for the week starting at this Monday. */
    val openWeekStartMs: Long? = null
)

/**
 * Backs the weeks ledger — the screen that replaced the swipe-through-weeks overlay (2026-08-23).
 * The overlay redrew the cardio hero's own marks one page away and could only be walked one week at
 * a time; a ledger shows the whole run at once, which is the read the weeks were being browsed for.
 *
 * The series runs from the first logged week to this one with no gaps, so a fallow stretch is visible
 * as fallow rather than closing up.
 */
@HiltViewModel
class CardioWeeksViewModel @Inject constructor(
    cardioRepo: CardioRepository,
    settingsRepo: SettingsRepository,
    private val clock: Clock
) : ViewModel() {

    private val openWeek = MutableStateFlow<Long?>(null)

    val state: StateFlow<CardioWeeksState> = combine(
        cardioRepo.observeAll(),
        settingsRepo.cardioWeeklyTargetMin,
        settingsRepo.useMiles,
        openWeek
    ) { entries, target, useMiles, open ->
        val zone = ZoneId.systemDefault()
        CardioWeeksState(
            loaded = true,
            weeks = cardioWeekSeries(entries, clock.nowMs(), weeksToCover(entries, zone), zone).reversed(),
            entries = entries,
            weekTargetMin = target,
            useMiles = useMiles,
            openWeekStartMs = open
        )
    }.flowOn(Dispatchers.Default)
        .stateIn(
            scope = viewModelScope,
            started = SharingStarted.WhileSubscribed(stopTimeoutMillis = 5_000),
            initialValue = CardioWeeksState()
        )

    fun openWeek(weekStartMs: Long) = openWeek.update { weekStartMs }
    fun closeWeek() = openWeek.update { null }

    /**
     * How many weeks back the ledger reaches: the first logged week, capped so a years-old history
     * doesn't build a list nobody scrolls. Always at least the window the overview's LOAD chart shows,
     * so arriving from `weeks →` never lands on a shorter run than the chart that sent you.
     */
    private fun weeksToCover(entries: List<CardioEntry>, zone: ZoneId): Int {
        val oldest = entries.minByOrNull { it.date } ?: return CardioViewModel.LOAD_WEEKS
        val oldestMonday = Instant.ofEpochMilli(oldest.date).atZone(zone).toLocalDate()
            .with(java.time.DayOfWeek.MONDAY)
        val currentMonday = Instant.ofEpochMilli(clock.nowMs()).atZone(zone).toLocalDate()
            .with(java.time.DayOfWeek.MONDAY)
        val span = ChronoUnit.WEEKS.between(oldestMonday, currentMonday).toInt() + 1
        return span.coerceIn(CardioViewModel.LOAD_WEEKS, MAX_WEEKS)
    }

    private companion object {
        /** Two years of weeks — past this the ledger is an archive, not a read. */
        const val MAX_WEEKS = 104
    }
}
