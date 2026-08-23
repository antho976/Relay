package com.forge.app.ui.cardio

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
import androidx.compose.foundation.lazy.items
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
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.cardio.CardioWeekPoint
import com.forge.app.domain.cardio.WHO_WEEKLY_ACTIVITY_MIN
import com.forge.app.domain.cardio.cardioWeekAggregate
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.toDisplayDistance
import com.forge.app.ui.cardio.components.CardioWeekDetail
import com.forge.app.ui.cardio.components.RankedBarRow
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.forgeItemMotion
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import java.util.Locale

/**
 * THE WEEKS — every week of cardio as a row, newest first (§3 List archetype: tiny hero, trim rows,
 * light stagger, no charts of its own).
 *
 * This replaced the swipe-through-weeks overlay (2026-08-23). That overlay drew the cardio hero's own
 * marks again one page away (§4.3), and could only be read one week per swipe — so the question it
 * existed to answer, "how do my weeks compare", was the one thing it could not show. Here each week
 * is a ranked bar against your biggest week, and the whole run reads at once.
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
    val zone = remember { ZoneId.systemDefault() }

    // A tapped week opens its own page — the same one back arrow, one level down (§4.6).
    val openWeek = state.openWeekStartMs
    if (openWeek != null) {
        CardioWeekDetail(
            weekStartMs = openWeek,
            agg = remember(state.entries, openWeek) { cardioWeekAggregate(state.entries, openWeek, zone) },
            weekEntries = remember(state.entries, openWeek) {
                val end = Instant.ofEpochMilli(openWeek).atZone(zone).toLocalDate()
                    .plusWeeks(1).atStartOfDay(zone).toInstant().toEpochMilli()
                state.entries.filter { it.date in openWeek until end }.sortedBy { it.date }
            },
            useMiles = state.useMiles,
            weekTargetMin = state.weekTargetMin,
            zone = zone,
            onOpenSession = onOpenSession,
            onBack = viewModel::closeWeek
        )
        return
    }

    val currentMonday = remember(zone) { LocalDate.now(zone).with(java.time.DayOfWeek.MONDAY) }
    // The scale every row's bar is drawn against — your biggest week, so a row reads as a share of
    // your best rather than of an arbitrary target.
    val peak = remember(state.weeks) { (state.weeks.maxOfOrNull { it.minutes } ?: 0).coerceAtLeast(1) }
    val target = if (state.weekTargetMin > 0) state.weekTargetMin else WHO_WEEKLY_ACTIVITY_MIN
    // Completed weeks only — the week still running has not had its chance to clear anything.
    val completed = remember(state.weeks) { state.weeks.drop(1) }
    val averageMin = remember(completed) { if (completed.isEmpty()) 0 else completed.sumOf { it.minutes } / completed.size }
    val cleared = remember(completed, target) { completed.count { it.minutes >= target } }

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
            // §3 List: a TINY hero — the name and at most two figures, no serif wall, no chart.
            item("hero") {
                Column(Modifier.padding(horizontal = 24.dp).padding(top = 4.dp)) {
                    Text("Weeks", style = MaterialTheme.typography.headlineSmall, color = onBg)
                    Spacer(Modifier.height(14.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                        EditorialFigure(
                            value = "$averageMin",
                            label = "min a week",
                            onBg = onBg, muted = muted, accent = accent,
                            modifier = Modifier.weight(1f)
                        )
                        EditorialFigure(
                            value = "$cleared",
                            label = "weeks cleared $target",
                            onBg = onBg, muted = muted, accent = accent,
                            modifier = Modifier.weight(1f)
                        )
                    }
                    Spacer(Modifier.height(20.dp))
                }
            }

            if (state.loaded && state.weeks.all { it.isEmpty }) {
                item("empty") {
                    InlineEmptyHint(
                        text = "Weeks fill in as you log sessions.",
                        color = muted,
                        modifier = Modifier.padding(horizontal = 24.dp)
                    )
                }
            }

            items(state.weeks, key = { it.weekStartMs }) { week ->
                WeekLedgerRow(
                    week = week,
                    currentMonday = currentMonday,
                    peak = peak,
                    useMiles = state.useMiles,
                    zone = zone,
                    onBg = onBg, muted = muted, outline = outline, accent = accent,
                    onClick = { viewModel.openWeek(week.weekStartMs) },
                    modifier = forgeItemMotion()
                )
            }
        }
    }
}

/**
 * One week in the ledger — its human name, its minutes and distance as the row's reading, and the
 * bar it fills against your biggest week. A fallow week keeps its row at an empty track rather than
 * vanishing, so a gap in training reads as a gap (§12).
 */
@Composable
private fun WeekLedgerRow(
    week: CardioWeekPoint,
    currentMonday: LocalDate,
    peak: Int,
    useMiles: Boolean,
    zone: ZoneId,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val start = remember(week.weekStartMs, zone) {
        Instant.ofEpochMilli(week.weekStartMs).atZone(zone).toLocalDate()
    }
    val name = remember(start, currentMonday) { relativeWeekLabel(start, currentMonday) }
    val meta = remember(week, useMiles) {
        buildList {
            add("${week.days} ${if (week.days == 1) "day" else "days"}")
            if (week.distanceKm > 0) {
                add(
                    String.format(Locale.US, "%.1f %s", toDisplayDistance(week.distanceKm, useMiles), distanceUnitLabel(useMiles))
                )
            }
        }.joinToString(" · ")
    }
    Column(
        modifier
            .fillMaxWidth()
            // The WHOLE row is the tap target (§2③).
            .clickableLabeled("Open $name") { onClick() }
            .padding(horizontal = 24.dp, vertical = 14.dp)
    ) {
        RankedBarRow(
            label = name,
            value = "${week.minutes} min",
            fraction = week.minutes.toFloat() / peak,
            onBg = onBg, muted = muted, outline = outline, accent = accent
        )
        if (meta.isNotBlank()) {
            Spacer(Modifier.height(6.dp))
            Text(
                meta.uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = muted, letterSpacing = 0.5.sp
            )
        }
    }
}

/** A machine week number never renders (§11) — a week reads as its human name or its date. */
internal fun relativeWeekLabel(weekStart: LocalDate, currentMonday: LocalDate): String =
    when (ChronoUnit.WEEKS.between(weekStart, currentMonday)) {
        0L -> "This week"
        1L -> "Last week"
        else -> "Week of ${weekStart.format(DateTimeFormatter.ofPattern("MMM d", Locale.getDefault()))}"
    }
