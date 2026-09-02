package com.forge.app.ui.cardio

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.cardio.CardioWeekPoint
import com.forge.app.domain.cardio.WHO_WEEKLY_ACTIVITY_MIN
import com.forge.app.domain.cardio.cardioWeekAggregate
import com.forge.app.ui.cardio.components.CardioWeekBars
import com.forge.app.ui.cardio.components.CardioWeekDetail
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.clickableLabeled
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import java.util.Locale

/**
 * THE WEEKS — one bar per week, taller the more you did that week; tap a bar to open it, and page
 * further back with the arrows.
 *
 * This replaced the swipe-through-weeks overlay (2026-08-23). That overlay drew the cardio hero's own
 * marks again one page away (§4.3) and could only be read one week per swipe, so the question it
 * existed to answer — how do my weeks compare — was the one thing it could not show. A chart answers
 * it at a glance, and the arrows reach back beyond what fits on screen.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CardioWeeksScreen(
    onBack: () -> Unit,
    onOpenSession: (Long) -> Unit = {},
    viewModel: CardioWeeksViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val accent = MaterialTheme.colorScheme.primary
    // The zone comes WITH the state, not from an unkeyed `remember` (M-15). A remembered zone
    // survives recomposition by definition, so after a flight the labels, the current-week test and
    // the detail's day ranges interpreted a new-zone epoch with the old zone's offset — a
    // day-boundary error that lands sessions in the wrong week, on a screen that is entirely about
    // which week something happened in. `CardioScreen` already reads it this way.
    val zone = state.zone

    // A tapped bar opens its own page — the same one back arrow, one level down (§4.6).
    val openWeek = state.openWeekStartMs
    // ...and system Back climbs that level exactly as the arrow does, including the distinction the
    // arrow already makes: entered ON a week, Back leaves; entered on the chart, Back returns to it.
    // Without this the gesture skipped the level entirely and popped the route from a week page.
    BackHandler(enabled = openWeek != null && !viewModel.arrivedOnWeek) { viewModel.closeWeek() }
    if (openWeek != null) {
        CardioWeekDetail(
            weekStartMs = openWeek,
            agg = remember(state.entries, openWeek, zone) { cardioWeekAggregate(state.entries, openWeek, zone) },
            weekEntries = remember(state.entries, openWeek, zone) {
                val end = Instant.ofEpochMilli(openWeek).atZone(zone).toLocalDate()
                    .plusWeeks(1).atStartOfDay(zone).toInstant().toEpochMilli()
                state.entries.filter { it.date in openWeek until end }.sortedBy { it.date }
            },
            useMiles = state.useMiles,
            weekTargetMin = state.weekTargetMin,
            zone = zone,
            todayStartMs = state.todayStartMs,
            onOpenSession = onOpenSession,
            // Entered ON this week → back leaves; entered on the chart → back returns to it.
            onBack = if (viewModel.arrivedOnWeek) onBack else viewModel::closeWeek
        )
        return
    }

    val perPage = CardioWeeksViewModel.WEEKS_PER_PAGE
    // 0 = the window ending on this week; each step back is one full page of older weeks.
    var pagesBack by remember { mutableIntStateOf(0) }
    val maxPagesBack = remember(state.weeks, perPage) {
        if (state.weeks.isEmpty()) 0 else (state.weeks.size - 1) / perPage
    }
    // Older data can land while the screen is open; clamp rather than stranding an out-of-range page.
    val page = pagesBack.coerceIn(0, maxPagesBack)
    val window = remember(state.weeks, page, perPage) {
        val end = (state.weeks.size - page * perPage).coerceAtLeast(0)
        val start = (end - perPage).coerceAtLeast(0)
        state.weeks.subList(start, end)
    }

    // Derived from the state's anchor, not from a clock read in this composition (M-15): the week
    // that is still running must stop being "current" the moment Monday arrives, on a chart that is
    // still open. Zero is the pre-load state only.
    val today = remember(state.todayStartMs, zone) {
        state.todayStartMs.takeIf { it > 0L }
            ?.let { Instant.ofEpochMilli(it).atZone(zone).toLocalDate() }
            ?: LocalDate.now(zone)
    }
    val currentWeekStartMs = remember(today, zone) {
        today.with(DayOfWeek.MONDAY).atStartOfDay(zone).toInstant().toEpochMilli()
    }
    val target = if (state.weekTargetMin > 0) state.weekTargetMin else WHO_WEEKLY_ACTIVITY_MIN
    // The figures read the WINDOW, so paging back actually says something about the weeks on screen.
    // The week still running is excluded — it has not had its chance to clear anything yet.
    val judged = remember(window, currentWeekStartMs) { window.filterNot { it.weekStartMs == currentWeekStartMs } }
    val averageMin = remember(judged) { if (judged.isEmpty()) 0 else judged.sumOf { it.minutes } / judged.size }
    val cleared = remember(judged, target) { judged.count { it.minutes >= target } }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {},
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = muted)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(bottom = 48.dp)
        ) {
            item("hero") {
                Column(Modifier.padding(horizontal = 24.dp).padding(top = 4.dp)) {
                    Text("Weeks", style = MaterialTheme.typography.headlineMedium, color = onBg)
                    Spacer(Modifier.height(16.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                        EditorialFigure(
                            value = "$averageMin",
                            label = "min a week",
                            onBg = onBg, muted = muted, accent = accent,
                            modifier = Modifier.weight(1f)
                        )
                        EditorialFigure(
                            value = "$cleared of ${judged.size}",
                            label = "weeks cleared $target",
                            onBg = onBg, muted = muted, accent = accent,
                            modifier = Modifier.weight(1f)
                        )
                    }
                }
            }

            item("nav") {
                Spacer(Modifier.height(24.dp))
                WeekRangeNav(
                    label = rangeLabel(window, zone, today.year),
                    // Page indices grow into the past, so "older" is bounded by maxPagesBack.
                    canGoOlder = page < maxPagesBack,
                    canGoNewer = page > 0,
                    onOlder = { pagesBack = (page + 1).coerceAtMost(maxPagesBack) },
                    onNewer = { pagesBack = (page - 1).coerceAtLeast(0) },
                    onBg = onBg, muted = muted, outline = outline
                )
            }

            item("chart") {
                Spacer(Modifier.height(20.dp))
                CardioWeekBars(
                    weeks = window,
                    targetMin = target,
                    currentWeekStartMs = currentWeekStartMs,
                    zone = zone,
                    onOpenWeek = viewModel::openWeek,
                    onBg = onBg, muted = muted, outline = outline, accent = accent,
                    modifier = Modifier.padding(horizontal = 24.dp)
                )
            }

            item("caption") {
                Spacer(Modifier.height(16.dp))
                Text(
                    // The one caption the chart is allowed (§4.3) — what the dashed rule across it means.
                    (if (state.weekTargetMin > 0) "Dashed line · your $target min target"
                    else "Dashed line · WHO $target min reference").uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    modifier = Modifier.padding(horizontal = 24.dp)
                )
            }
        }
    }
}

/**
 * The pager: `←` older · the visible range · `→` newer. An arrow that cannot move renders passive —
 * nothing looks tappable while doing nothing (§4.5) — and its touch target comes from padding, not
 * from glyph size (§14).
 */
@Composable
private fun WeekRangeNav(
    label: String,
    canGoOlder: Boolean,
    canGoNewer: Boolean,
    onOlder: () -> Unit,
    onNewer: () -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color
) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(
            "←",
            style = MaterialTheme.typography.titleMedium,
            color = if (canGoOlder) onBg else outline.copy(alpha = 0.35f),
            modifier = Modifier
                .then(if (canGoOlder) Modifier.clickableLabeled("Earlier weeks", onClick = onOlder) else Modifier)
                .padding(14.dp)
        )
        Text(label.uppercase(), style = MaterialTheme.typography.labelMedium, color = muted)
        Text(
            "→",
            style = MaterialTheme.typography.titleMedium,
            color = if (canGoNewer) onBg else outline.copy(alpha = 0.35f),
            modifier = Modifier
                .then(if (canGoNewer) Modifier.clickableLabeled("Later weeks", onClick = onNewer) else Modifier)
                .padding(14.dp)
        )
    }
}

/**
 * "18 Aug – 12 Oct" for the visible window; the year only when it is not this one.
 *
 * [thisYear] is passed rather than read from the clock here (M-15), for the same reason the zone
 * is: this runs inside a composition, and a composition that reads the clock has nothing to
 * recompose it. Over New Year the label kept suppressing a year suffix it now needs.
 */
private fun rangeLabel(window: List<CardioWeekPoint>, zone: ZoneId, thisYear: Int): String {
    if (window.isEmpty()) return ""
    val fmt = DateTimeFormatter.ofPattern("d MMM", Locale.getDefault())
    val first = Instant.ofEpochMilli(window.first().weekStartMs).atZone(zone).toLocalDate()
    val last = Instant.ofEpochMilli(window.last().weekStartMs).atZone(zone).toLocalDate().plusDays(6)
    val yearSuffix = if (last.year != thisYear) " ${last.year}" else ""
    return "${first.format(fmt)} – ${last.format(fmt)}$yearSuffix"
}

/** A machine week number never renders (§11) — a week reads as its human name or its date. */
internal fun relativeWeekLabel(weekStart: LocalDate, currentMonday: LocalDate): String =
    when (ChronoUnit.WEEKS.between(weekStart, currentMonday)) {
        0L -> "This week"
        1L -> "Last week"
        else -> "Week of ${weekStart.format(DateTimeFormatter.ofPattern("MMM d", Locale.getDefault()))}"
    }
