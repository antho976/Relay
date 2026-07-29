package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.cardioWeekAggregate
import com.forge.app.ui.common.clickableLabeled
import kotlinx.coroutines.launch
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import java.util.Locale

/**
 * The stats overlay — one week per page, swipe left/right to walk through earlier weeks. The current
 * week sits at the last page (rightmost); swiping back (finger left→right) flips to older weeks, like
 * a calendar. Each page is a [CardioWeekStatsPage] computed on the fly from the in-memory entry list,
 * so no extra DB reads are needed to browse history.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CardioWeekDetailSheet(
    allEntries: List<CardioEntry>,
    currentWeekStartMs: Long,
    useMiles: Boolean,
    weekTargetMin: Int,
    cardioStreakDays: Int,
    /** Per-activity pace series (GYMAP-35) — rendered only on the current-week page (cross-week data). */
    paceSeries: List<com.forge.app.domain.cardio.CardioPaceSeries>,
    /** Watch-derived steps/route for the current week (today); null when none loaded. */
    wearable: CardioWearableDay?,
    /** Avex holds the steps grant — show the current-week steps section even before data syncs. */
    wearableConnected: Boolean,
    todayDow: Int,
    zone: ZoneId,
    onOpenSession: (Long) -> Unit,
    onBack: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val accent = MaterialTheme.colorScheme.primary

    val currentMonday = remember(currentWeekStartMs) {
        Instant.ofEpochMilli(currentWeekStartMs).atZone(zone).toLocalDate()
    }
    // How many weeks of history to page through — from the oldest entry's week up to this week.
    val pageCount = remember(allEntries, currentMonday) {
        val oldest = allEntries.minByOrNull { it.date } ?: return@remember 1
        val oldestMonday = Instant.ofEpochMilli(oldest.date).atZone(zone).toLocalDate()
            .with(java.time.DayOfWeek.MONDAY)
        (ChronoUnit.WEEKS.between(oldestMonday, currentMonday).toInt().coerceAtLeast(0)) + 1
    }
    val pagerState = rememberPagerState(initialPage = pageCount - 1, pageCount = { pageCount })
    val scope = rememberCoroutineScope()

    // If older entries arrive while the overlay is open, pageCount grows and older weeks are prepended
    // (lower indices) — which would silently shift the visible page to a different calendar week. Shift
    // the current page by the delta so the SAME week stays in view (currentMonday is fixed, so growth
    // only ever adds older weeks at the front).
    var lastPageCount by remember { mutableIntStateOf(pageCount) }
    LaunchedEffect(pageCount) {
        if (pageCount != lastPageCount) {
            val target = (pagerState.currentPage + (pageCount - lastPageCount)).coerceIn(0, pageCount - 1)
            pagerState.scrollToPage(target)
            lastPageCount = pageCount
        }
    }

    // Page index → weeks-ago: the last page is the current week (0 weeks ago).
    fun weeksAgoFor(page: Int) = pageCount - 1 - page
    fun weekStartFor(page: Int): LocalDate = currentMonday.minusWeeks(weeksAgoFor(page).toLong())

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
        Column(Modifier.fillMaxSize().padding(inner)) {
            // Week navigator header — reflects the settled/target page, with tappable chevrons.
            val headerPage = pagerState.targetPage
            val headerStart = weekStartFor(headerPage)
            val canGoOlder = headerPage > 0
            val canGoNewer = headerPage < pageCount - 1
            Row(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Text(
                    "←",
                    style = MaterialTheme.typography.titleMedium,
                    color = if (canGoOlder) onBg else outline.copy(alpha = 0.35f),
                    modifier = Modifier
                        .then(
                            if (canGoOlder) Modifier.clickableLabeled("Earlier week") {
                                scope.launch { pagerState.animateScrollToPage(headerPage - 1) }
                            } else Modifier
                        )
                        // Padding, not glyph size, carries the ≥48dp touch target (§8).
                        .padding(14.dp)
                )
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(relativeWeekLabel(headerStart, currentMonday), style = MaterialTheme.typography.titleMedium, color = onBg)
                    Text(weekRangeLabel(headerStart), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                }
                Text(
                    "→",
                    style = MaterialTheme.typography.titleMedium,
                    color = if (canGoNewer) onBg else outline.copy(alpha = 0.35f),
                    modifier = Modifier
                        .then(
                            if (canGoNewer) Modifier.clickableLabeled("Later week") {
                                scope.launch { pagerState.animateScrollToPage(headerPage + 1) }
                            } else Modifier
                        )
                        .padding(14.dp)
                )
            }

            HorizontalPager(state = pagerState, modifier = Modifier.fillMaxSize()) { page ->
                val weekStart = weekStartFor(page)
                val weekStartMs = weekStart.atStartOfDay(zone).toInstant().toEpochMilli()
                val weekEndMs = weekStart.plusDays(7).atStartOfDay(zone).toInstant().toEpochMilli()
                val agg = remember(allEntries, weekStartMs) { cardioWeekAggregate(allEntries, weekStartMs, zone) }
                val weekEntries = remember(allEntries, weekStartMs) {
                    allEntries.filter { it.date in weekStartMs until weekEndMs && it.type != CardioType.REST.code }
                }
                CardioWeekStatsPage(
                    agg = agg,
                    weekEntries = weekEntries,
                    useMiles = useMiles,
                    isCurrentWeek = weeksAgoFor(page) == 0,
                    // Cross-week pace trend belongs to the current-week page alone, so it never repeats (§4.3).
                    paceSeries = if (weeksAgoFor(page) == 0) paceSeries else emptyList(),
                    todayDow = todayDow,
                    weekTargetMin = weekTargetMin,
                    cardioStreakDays = cardioStreakDays,
                    wearable = if (weeksAgoFor(page) == 0) wearable else null,
                    // Steps are only loaded for the current week, so the placeholder belongs there too.
                    wearableConnected = weeksAgoFor(page) == 0 && wearableConnected,
                    zone = zone,
                    onOpenSession = onOpenSession,
                    onBg = onBg, muted = muted, outline = outline, accent = accent
                )
            }
        }
    }
}

private fun relativeWeekLabel(weekStart: LocalDate, currentMonday: LocalDate): String =
    when (ChronoUnit.WEEKS.between(weekStart, currentMonday)) {
        0L -> "This week"
        1L -> "Last week"
        // A raw ISO week number never renders (§11) — older weeks read as their human date.
        else -> "Week of ${weekStart.format(DateTimeFormatter.ofPattern("MMM d", Locale.getDefault()))}"
    }

private fun weekRangeLabel(weekStart: LocalDate): String {
    val fmt = DateTimeFormatter.ofPattern("MMM d", Locale.getDefault())
    return "${weekStart.format(fmt)} – ${weekStart.plusDays(6).format(fmt)}".uppercase()
}
