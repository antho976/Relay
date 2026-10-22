package com.forge.app.ui.cardio

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.cardio.CardioWeekPoint
import com.forge.app.domain.cardio.WHO_WEEKLY_ACTIVITY_MIN
import com.forge.app.domain.cardio.cardioWeekAggregate
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.ui.cardio.components.CardioWeekDetail
import com.forge.app.ui.common.OpenColumnChart
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.theme.CardioOpenType
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle as JavaTextStyle
import java.time.temporal.ChronoUnit
import java.util.Locale
import kotlin.math.roundToInt

/**
 * WEEKS (redesign 2026-10-21) — from the hub's "Weeks ›". One bar per week against the weekly mark
 * (red = reached it, the week still running dim), paged eight at a time with the arrows, the share of
 * weeks on target and the average week over the window, then the same weeks as rows with their own
 * day bars. Tap a bar or a row for that week's own page.
 */
@Composable
fun CardioWeeksScreen(
    onBack: () -> Unit,
    onOpenSession: (Long) -> Unit = {},
    viewModel: CardioWeeksViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    // The zone travels with the state (M-15), never remembered here.
    val zone = state.zone

    val openWeek = state.openWeekStartMs
    // Above the week page's early return, so closing a week lands back on the page it was opened from.
    var pagesBack by rememberSaveable { mutableStateOf(0) }
    // Entered ON a week, Back leaves; entered on the chart, Back returns to it.
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
            onBack = if (viewModel.arrivedOnWeek) onBack else viewModel::closeWeek
        )
        return
    }
    CardioWeeksContent(
        state = state,
        pagesBack = pagesBack,
        onPage = { pagesBack = it },
        onOpenWeek = viewModel::openWeek,
        onBack = onBack
    )
}

/** What the page reads over its window, worked out without a composition so a test can pin it. */
internal data class WeeksRead(
    /** The window's weeks that can be judged: lived, and not the one still running. */
    val judged: Int,
    val onTarget: Int,
    val averageMin: Int,
    /** The average week against the window before it, in percent; null with nothing to compare. */
    val changePct: Int?,
)

internal fun weeksRead(
    window: List<CardioWeekPoint>,
    before: List<CardioWeekPoint>,
    targetMin: Int,
    currentWeekStartMs: Long,
    firstLoggedWeekMs: Long?,
): WeeksRead {
    fun judgedOf(list: List<CardioWeekPoint>) = list.filter {
        it.weekStartMs != currentWeekStartMs && firstLoggedWeekMs != null && it.weekStartMs >= firstLoggedWeekMs
    }
    val judged = judgedOf(window)
    val avg = if (judged.isEmpty()) 0 else judged.sumOf { it.minutes } / judged.size
    val prev = judgedOf(before)
    val prevAvg = if (prev.isEmpty()) 0 else prev.sumOf { it.minutes } / prev.size
    return WeeksRead(
        judged = judged.size,
        onTarget = judged.count { it.minutes >= targetMin },
        averageMin = avg,
        changePct = if (prev.isEmpty() || prevAvg <= 0 || judged.isEmpty()) null
        else ((avg - prevAvg) * 100.0 / prevAvg).roundToInt(),
    )
}

/** A week row's status: Running, On target, Short N, or Not tracked before the first log. */
internal fun weekStatus(week: CardioWeekPoint, targetMin: Int, currentWeekStartMs: Long, firstLoggedWeekMs: Long?): Pair<String, Boolean> = when {
    week.weekStartMs == currentWeekStartMs -> "Running" to false
    firstLoggedWeekMs == null || week.weekStartMs < firstLoggedWeekMs -> "Not tracked" to false
    week.minutes >= targetMin -> "On target" to true
    else -> "Short ${targetMin - week.minutes}" to false
}

/** The weeks page drawn from its state alone. [pagesBack] 0 is the window ending on this week. */
@Composable
internal fun CardioWeeksContent(
    state: CardioWeeksState,
    pagesBack: Int,
    onPage: (Int) -> Unit,
    onOpenWeek: (Long) -> Unit,
    onBack: () -> Unit,
) {
    val zone = state.zone
    val locale = currentLocale()
    val accent = MaterialTheme.colorScheme.primary
    val perPage = CardioWeeksViewModel.WEEKS_PER_PAGE
    val maxPagesBack = if (state.weeks.isEmpty()) 0 else (state.weeks.size - 1) / perPage
    val page = pagesBack.coerceIn(0, maxPagesBack)
    val window = remember(state.weeks, page) {
        val end = (state.weeks.size - page * perPage).coerceAtLeast(0)
        state.weeks.subList((end - perPage).coerceAtLeast(0), end)
    }
    val before = remember(state.weeks, page) {
        val end = (state.weeks.size - (page + 1) * perPage).coerceAtLeast(0)
        state.weeks.subList((end - perPage).coerceAtLeast(0), end)
    }
    val today = remember(state.todayStartMs, zone) {
        state.todayStartMs.takeIf { it > 0L }?.let { Instant.ofEpochMilli(it).atZone(zone).toLocalDate() } ?: LocalDate.now(zone)
    }
    val currentWeekStartMs = remember(today, zone) { today.with(DayOfWeek.MONDAY).atStartOfDay(zone).toInstant().toEpochMilli() }
    val personal = state.weekTargetMin > 0
    val target = if (personal) state.weekTargetMin else WHO_WEEKLY_ACTIVITY_MIN
    // Weeks before the first entry were never lived in Avex, so they are not judged (see weeksRead).
    val firstLoggedWeekMs = remember(state.entries, zone) {
        state.entries.minOfOrNull { it.date }?.let {
            Instant.ofEpochMilli(it).atZone(zone).toLocalDate().with(DayOfWeek.MONDAY).atStartOfDay(zone).toInstant().toEpochMilli()
        }
    }
    val read = remember(window, before, target, currentWeekStartMs, firstLoggedWeekMs) {
        weeksRead(window, before, target, currentWeekStartMs, firstLoggedWeekMs)
    }
    fun dateOf(ms: Long): LocalDate = Instant.ofEpochMilli(ms).atZone(zone).toLocalDate()
    val rangeCaption = remember(window, locale) {
        if (window.isEmpty()) "" else {
            val first = dateOf(window.first().weekStartMs)
            val last = dateOf(window.last().weekStartMs).plusDays(6)
            val md = DateTimeFormatter.ofPattern("MMM d", locale)
            val year = if (last.year != today.year) ", ${last.year}" else ""
            "${first.format(md)} to ${last.format(md)}$year · ${window.size} weeks"
        }
    }
    val unit = distanceUnitLabel(state.useMiles)

    LazyColumn(
        modifier = Modifier.fillMaxSize().statusBarsPadding(),
        contentPadding = PaddingValues(start = CARDIO_GUTTER, end = CARDIO_GUTTER, top = 4.dp, bottom = 56.dp)
    ) {
        item("top") { CardioTopRow(onBack = onBack) }
        item("title") {
            Column {
                Spacer(Modifier.height(20.dp))
                Text("Weeks", style = OpenType.titleL, color = MaterialTheme.colorScheme.onBackground)
                Spacer(Modifier.height(10.dp))
                CardioChange(rangeCaption, moved = false)
                Spacer(Modifier.height(28.dp))
            }
        }
        item("figures") {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                CardioReading(
                    label = "On target",
                    figure = cardioFigure("${read.onTarget}", "/${read.judged}", OpenType.figureM),
                    change = "Weeks at $target min",
                    moved = false,
                    description = "${read.onTarget} of ${read.judged} weeks reached $target minutes",
                    modifier = Modifier.weight(1f)
                )
                val change = read.changePct?.let { p ->
                    when {
                        p > 0 -> "Minutes · ↑ $p%"
                        p < 0 -> "Minutes · ↓ ${-p}%"
                        else -> "Minutes · level"
                    }
                } ?: "Minutes"
                CardioReading(
                    label = "Avg week",
                    figure = cardioFigure("${read.averageMin}", " min", CardioOpenType.figureMUnit),
                    change = change,
                    moved = (read.changePct ?: 0) != 0,
                    description = "An average week of ${read.averageMin} minutes, $change",
                    modifier = Modifier.weight(1f)
                )
            }
        }
        item("pager") {
            Column {
                Spacer(Modifier.height(24.dp))
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    PagerArrow(OpenIcons.ChevronLeft, "Earlier weeks", page < maxPagesBack) { onPage((page + 1).coerceAtMost(maxPagesBack)) }
                    Text(
                        "${window.size} weeks".uppercase(),
                        style = OpenType.monoXS,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                        modifier = Modifier.weight(1f).padding(horizontal = 8.dp)
                    )
                    PagerArrow(OpenIcons.ChevronRight, "Later weeks", page > 0) { onPage((page - 1).coerceAtLeast(0)) }
                }
            }
        }
        item("chart") {
            Column {
                Spacer(Modifier.height(10.dp))
                Box(Modifier.fillMaxWidth()) {
                    OpenColumnChart(
                        values = window.map { it.minutes.toFloat() },
                        colors = window.map {
                            when {
                                it.weekStartMs == currentWeekStartMs -> accent.copy(alpha = 0.35f)
                                it.minutes >= target -> accent
                                else -> OpenColors.Bar
                            }
                        },
                        height = 180.dp,
                        columnLabels = window.map { "${dateOf(it.weekStartMs).dayOfMonth}" },
                        valueLabels = true,
                        gapFraction = 0.45f,
                        description = "Minutes per week against $target. " +
                            window.joinToString(". ") { "Week of ${dateOf(it.weekStartMs).format(DateTimeFormatter.ofPattern("MMM d", locale))}, ${it.minutes} minutes" }
                    )
                    // One tap target per bar, over the chart: each opens its week.
                    Row(Modifier.matchParentSize()) {
                        window.forEach { w ->
                            Box(
                                Modifier.weight(1f).fillMaxHeight()
                                    .clickableLabeled("Open the week of ${dateOf(w.weekStartMs).format(DateTimeFormatter.ofPattern("MMM d", locale))}") {
                                        onOpenWeek(w.weekStartMs)
                                    }
                            )
                        }
                    }
                }
                Spacer(Modifier.height(8.dp))
                CardioChange(
                    if (personal) "Red weeks reached your $target · this week still running"
                    else "Red weeks reached $target · this week still running",
                    moved = false
                )
            }
        }
        item("rows-head") {
            Column {
                Spacer(Modifier.height(40.dp))
                CardioHead("By week", "Minutes")
                Spacer(Modifier.height(4.dp))
            }
        }
        items(window.reversed(), key = { it.weekStartMs }) { w ->
            val d = dateOf(w.weekStartMs)
            val (status, ink) = weekStatus(w, target, currentWeekStartMs, firstLoggedWeekMs)
            val lit = w.weekStartMs == currentWeekStartMs || w.minutes >= target
            val dist = distanceNumber(w.distanceKm, state.useMiles)
            val sessions = "${w.sessions} ${if (w.sessions == 1) "session" else "sessions"} · $dist $unit"
            val month = d.month.getDisplayName(JavaTextStyle.SHORT, locale)
            Row(
                Modifier
                    .fillMaxWidth()
                    .clickableLabeled("Open the week of $month ${d.dayOfMonth}") { onOpenWeek(w.weekStartMs) }
                    .padding(vertical = 11.dp)
                    .semantics(mergeDescendants = true) {},
                horizontalArrangement = Arrangement.spacedBy(14.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                CardioDateColumn(month, "${d.dayOfMonth}")
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    CardioMiniDayBars(w.perDayMinutes, lit)
                    OpenLabel(sessions, style = OpenType.monoXS)
                }
                Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    CardioRowFigure("${w.minutes}", " min", small = true)
                    CardioChange(status, moved = ink)
                }
            }
        }
    }
}

/** A pager chevron: 48dp to the finger, dimmed and passive when it cannot move. */
@Composable
private fun PagerArrow(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, enabled: Boolean, onClick: () -> Unit) {
    val tint = MaterialTheme.colorScheme.onSurfaceVariant
    Box(
        Modifier
            .touchSlop(vertical = 12.dp, horizontal = 12.dp)
            .then(
                if (enabled) Modifier.clickableLabeled(label, onClick = onClick)
                else Modifier.semantics { contentDescription = label; disabled() }
            )
            .padding(12.dp)
    ) {
        Icon(icon, contentDescription = if (enabled) label else null, tint = if (enabled) tint else tint.copy(alpha = 0.35f), modifier = Modifier.size(16.dp))
    }
}

/** A machine week number never renders (§11) — a week reads as its human name or its date. */
internal fun relativeWeekLabel(weekStart: LocalDate, currentMonday: LocalDate): String =
    when (ChronoUnit.WEEKS.between(weekStart, currentMonday)) {
        0L -> "This week"
        1L -> "Last week"
        else -> "Week of ${weekStart.format(DateTimeFormatter.ofPattern("MMM d", Locale.getDefault()))}"
    }
