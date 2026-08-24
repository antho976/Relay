package com.forge.app.ui.cardio

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.core.time.Clock
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.CardioRepository
import com.forge.app.domain.cardio.CardioWeekPoint
import com.forge.app.domain.cardio.cardioWeekSeries
import com.forge.app.ui.nav.Routes
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

/** State for the weeks chart — one bar per week, oldest first. */
data class CardioWeeksState(
    val loaded: Boolean = false,
    /** Oldest→newest, the chart's drawing order. Untrained weeks are kept at zero, so a fallow
     *  stretch reads as fallow instead of closing up. */
    val weeks: List<CardioWeekPoint> = emptyList(),
    /** Every entry, so an opened week can list its own sessions with no second query. */
    val entries: List<CardioEntry> = emptyList(),
    val weekTargetMin: Int = 0,
    val useMiles: Boolean = false,
    /** Non-null → the week detail is open for the week starting at this Monday. */
    val openWeekStartMs: Long? = null
)

/**
 * Backs the weeks chart — the screen that replaced the swipe-through-weeks overlay (2026-08-23).
 * The overlay redrew the cardio hero's own marks one page away and could only be walked one week per
 * swipe; a bar per week compares the whole run at a glance, which is the read weeks were browsed for.
 *
 * The series runs from the first logged week to this one with no gaps, and the screen pages a window
 * of it with the arrows.
 */
@HiltViewModel
class CardioWeeksViewModel @Inject constructor(
    cardioRepo: CardioRepository,
    settingsRepo: SettingsRepository,
    savedState: SavedStateHandle,
    private val clock: Clock
) : ViewModel() {

    // Arriving with a week argument (the cardio hero's Mon–Sun strip) opens straight into that week;
    // arriving without one (`weeks →`) lands on the chart. Backing out of the week shows the chart
    // either way, so the strip is a shortcut into the same place rather than a separate screen.
    private val openWeek = MutableStateFlow(
        savedState.get<Long>(Routes.ARG_WEEK_START)?.takeIf { it > 0L }
    )

    /**
     * True when the screen was ENTERED on a week (the hero's strip) rather than on the chart. Backing
     * out of that week leaves the route entirely — landing on a chart you never asked for would be a
     * surprise, and the strip is meant to be a shortcut, not a detour.
     */
    val arrivedOnWeek: Boolean = openWeek.value != null

    val state: StateFlow<CardioWeeksState> = combine(
        cardioRepo.observeAll(),
        settingsRepo.cardioWeeklyTargetMin,
        settingsRepo.useMiles,
        openWeek
    ) { entries, target, useMiles, open ->
        val zone = ZoneId.systemDefault()
        CardioWeeksState(
            loaded = true,
            weeks = cardioWeekSeries(entries, clock.nowMs(), weeksToCover(entries, zone), zone),
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
     * How many weeks back the chart reaches: the first logged week, capped so a years-old history
     * doesn't page forever. Always at least one full window, so the arrows have somewhere to go and
     * a fresh install still draws a chart at zero rather than a stub.
     */
    private fun weeksToCover(entries: List<CardioEntry>, zone: ZoneId): Int {
        val oldest = entries.minByOrNull { it.date } ?: return WEEKS_PER_PAGE
        val oldestMonday = Instant.ofEpochMilli(oldest.date).atZone(zone).toLocalDate()
            .with(java.time.DayOfWeek.MONDAY)
        val currentMonday = Instant.ofEpochMilli(clock.nowMs()).atZone(zone).toLocalDate()
            .with(java.time.DayOfWeek.MONDAY)
        val span = ChronoUnit.WEEKS.between(oldestMonday, currentMonday).toInt() + 1
        return span.coerceIn(WEEKS_PER_PAGE, MAX_WEEKS)
    }

    companion object {
        /** Weeks drawn per page. Eight bars still read across the 24dp page gutter on a small phone. */
        const val WEEKS_PER_PAGE = 8

        /** Two years of weeks — past this the chart is an archive, not a read. */
        private const val MAX_WEEKS = 104
    }
}
