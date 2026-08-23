package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
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
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioWeekAggregate
import com.forge.app.domain.cardio.WHO_WEEKLY_ACTIVITY_MIN
import com.forge.app.domain.cardio.pacePerUnit
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.toDisplayDistance
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.InlineEmptyHint
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

/**
 * ONE week, scoped to that week alone (§3 Detail archetype). Opened from a row of the weeks ledger.
 *
 * It is the old week-pager page with the pager taken off it: no swiping between weeks (the ledger is
 * the browser now), no sections that only ever rendered on the current week, and no `HorizontalDivider`
 * between the session rows — §1 spends a line on data, and air separates sections.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CardioWeekDetail(
    weekStartMs: Long,
    agg: CardioWeekAggregate,
    /** That week's active sessions, oldest first. */
    weekEntries: List<CardioEntry>,
    useMiles: Boolean,
    weekTargetMin: Int,
    zone: ZoneId,
    onOpenSession: (Long) -> Unit,
    onBack: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val accent = MaterialTheme.colorScheme.primary

    val weekStart = remember(weekStartMs, zone) {
        Instant.ofEpochMilli(weekStartMs).atZone(zone).toLocalDate()
    }
    val currentMonday = remember(zone) { LocalDate.now(zone).with(java.time.DayOfWeek.MONDAY) }
    val isCurrentWeek = weekStart == currentMonday
    val todayDow = if (isCurrentWeek) LocalDate.now(zone).dayOfWeek.value - 1 else -1
    val title = remember(weekStart, currentMonday) {
        com.forge.app.ui.cardio.relativeWeekLabel(weekStart, currentMonday)
    }
    val range = remember(weekStart) {
        val fmt = DateTimeFormatter.ofPattern("MMM d", Locale.getDefault())
        "${weekStart.format(fmt)} – ${weekStart.plusDays(6).format(fmt)}".uppercase()
    }

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
            // §3 Detail: a serif title over its context line — never a top-bar title (§4.6).
            item("hero") {
                Column(Modifier.padding(horizontal = 24.dp).padding(top = 4.dp)) {
                    Text(range, style = MaterialTheme.typography.labelSmall, color = muted, letterSpacing = 1.sp)
                    Spacer(Modifier.height(6.dp))
                    Text(title, style = MaterialTheme.typography.headlineMedium, color = onBg)
                    Spacer(Modifier.height(18.dp))
                    PerDayBars(
                        perDayMinutes = agg.perDayMinutes,
                        todayDow = todayDow,
                        onBg = onBg, muted = muted, outline = outline, accent = accent
                    )
                    Spacer(Modifier.height(20.dp))
                }
            }

            item("figures") {
                val avgPace = pacePerUnit(agg.minutes, agg.distanceKm, useMiles)
                val distUnit = distanceUnitLabel(useMiles)
                // Honest zeros, never a dash and never hidden (§12).
                val figures = buildList {
                    add("${agg.days}" to if (agg.days == 1) "day" else "days")
                    add("${agg.sessions}" to "sessions")
                    add("${agg.minutes}" to "minutes")
                    if (agg.distanceKm > 0) {
                        add(String.format(Locale.US, "%.1f", toDisplayDistance(agg.distanceKm, useMiles)) to distUnit)
                    }
                    if (avgPace != null) add(avgPace to "/$distUnit avg")
                }
                Column(Modifier.padding(horizontal = 24.dp)) {
                    figures.chunked(3).forEach { rowFigs ->
                        Row(Modifier.fillMaxWidth()) {
                            rowFigs.forEachIndexed { idx, (value, label) ->
                                EditorialFigure(
                                    value = value,
                                    label = label,
                                    onBg = onBg, muted = muted, accent = accent,
                                    modifier = Modifier.weight(1f)
                                )
                                // Whitespace separates figures — a line is data only (§1).
                                if (idx < rowFigs.lastIndex) Spacer(Modifier.width(20.dp))
                            }
                            repeat(3 - rowFigs.size) { Spacer(Modifier.weight(1f)) }
                        }
                        Spacer(Modifier.height(16.dp))
                    }
                    // The meter reads every week, not just the current one — a finished week that fell
                    // short is exactly the week you opened this page to look at.
                    val hasGoal = weekTargetMin > 0
                    val target = if (hasGoal) weekTargetMin else WHO_WEEKLY_ACTIVITY_MIN
                    MeterBar(
                        fraction = agg.minutes.toFloat() / target,
                        caption = when {
                            agg.minutes >= target && hasGoal -> "Goal hit · ${agg.minutes} of $target min"
                            hasGoal -> "${agg.minutes} of $target min"
                            agg.minutes >= target -> "WHO 150 min · met"
                            else -> "${agg.minutes} of 150 min · WHO reference"
                        },
                        muted = muted, outline = outline, accent = accent,
                        contentDescription = "${agg.minutes} of $target minutes this week"
                    )
                }
            }

            if (agg.minutesByType.isNotEmpty()) {
                item("by-activity") {
                    Spacer(Modifier.height(28.dp))
                    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
                        EditorialHeader(label = "By activity", muted = muted, accent = accent)
                        Spacer(Modifier.height(12.dp))
                        val leader = agg.minutesByType.first().second.coerceAtLeast(1)
                        Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                            agg.minutesByType.take(4).forEach { (type, min) ->
                                RankedBarRow(
                                    label = type.displayName,
                                    value = "$min min",
                                    fraction = min.toFloat() / leader,
                                    onBg = onBg, muted = muted, outline = outline, accent = accent
                                )
                            }
                        }
                    }
                }
            }

            item("sessions-header") {
                Spacer(Modifier.height(28.dp))
                EditorialHeader(
                    label = "Sessions",
                    muted = muted,
                    accent = accent,
                    modifier = Modifier.padding(horizontal = 24.dp)
                )
                Spacer(Modifier.height(8.dp))
            }
            if (weekEntries.isEmpty()) {
                item("sessions-empty") {
                    // The all-zero day bars above already drew the empty week; this is the last-resort
                    // line, and it replaces the caption rather than joining one (§12).
                    InlineEmptyHint(
                        text = "Nothing logged this week.",
                        color = muted,
                        modifier = Modifier.padding(horizontal = 24.dp)
                    )
                }
            }
            items(weekEntries.size, key = { weekEntries[it].id }) { i ->
                val entry = weekEntries[i]
                SessionTimelineRow(
                    entry = entry, useMiles = useMiles, zone = zone,
                    onBg = onBg, muted = muted,
                    onClick = { onOpenSession(entry.id) }
                )
            }
        }
    }
}

/** The week's Mon–Sun minutes. An untrained day keeps a ghost track mark rather than nothing (§12). */
@Composable
private fun PerDayBars(
    perDayMinutes: List<Int>,
    todayDow: Int,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color
) {
    val letters = listOf("M", "T", "W", "T", "F", "S", "S")
    val maxMin = (perDayMinutes.maxOrNull() ?: 0).coerceAtLeast(1)
    val reading = remember(perDayMinutes) {
        "${perDayMinutes.count { it > 0 }} of 7 days trained, ${perDayMinutes.sum()} minutes"
    }
    VerticalBarRow(
        count = 7,
        trackHeight = 56.dp,
        modifier = Modifier
            .fillMaxWidth()
            .semantics(mergeDescendants = true) { contentDescription = reading },
        bar = { i ->
            val mins = perDayMinutes.getOrElse(i) { 0 }
            val frac = (mins.toFloat() / maxMin).coerceIn(0f, 1f)
            if (mins > 0) BarGeom(height = (8 + 48 * frac).dp, fill = accent)
            else BarGeom(height = 4.dp, fill = outline.copy(alpha = 0.35f))
        },
        top = { i ->
            val mins = perDayMinutes.getOrElse(i) { 0 }
            if (mins > 0) Text("${mins}m", style = MaterialTheme.typography.labelSmall, color = onBg, fontWeight = FontWeight.SemiBold)
        },
        bottom = { i ->
            Text(
                letters[i], style = MaterialTheme.typography.labelSmall,
                color = if (i == todayDow) onBg else muted,
                fontWeight = if (i == todayDow) FontWeight.Bold else FontWeight.Normal
            )
        }
    )
}
