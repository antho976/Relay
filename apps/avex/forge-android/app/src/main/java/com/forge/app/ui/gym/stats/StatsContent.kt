package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.WeekFields
import java.util.Locale

/**
 * Gym → Stats tab. A segmented sub-nav (Snapshot · Strength · Volume · Trends) switches
 * between focused stat views; each view's content lives in StatsTabs.kt as a
 * LazyListScope builder so this file stays a thin scaffold.
 */
@Composable
fun StatsContent(
    modifier: Modifier = Modifier,
    onOpenHistory: () -> Unit = {},
    onOpenNotes: () -> Unit = {},
    onOpenRecap: () -> Unit = {},
    viewModel: StatsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    val zone = ZoneId.systemDefault()
    val today = LocalDate.now(zone)
    val weekNum = today.get(WeekFields.ISO.weekOfWeekBasedYear())
    val isoWeekStart = today.minusDays(today.dayOfWeek.value.toLong() - 1)
    val isoWeekEnd = isoWeekStart.plusDays(6)
    val weekLabel = remember(weekNum) {
        val fmt = DateTimeFormatter.ofPattern("MMM d", Locale.getDefault())
        "${isoWeekStart.format(fmt).uppercase()} — ${isoWeekEnd.format(fmt).uppercase()}"
    }

    val c = StatsColors(
        onBg = MaterialTheme.colorScheme.onBackground,
        muted = MaterialTheme.colorScheme.onSurfaceVariant,
        accent = MaterialTheme.colorScheme.primary,
        outline = MaterialTheme.colorScheme.outline
    )

    val weekCurrent = state.weekComparison?.current
    val weekPrev = state.weekComparison?.previous
    val weekSessions = weekCurrent?.sessions ?: 0

    // A pager (not a tap-only switch) so tabs can be swiped, with free physics-based page
    // transitions. The tab bar's sliding pill tracks the settled page; tapping a tab scrolls
    // the pager to it.
    val tabs = StatsTab.entries
    val pagerState = rememberPagerState(pageCount = { tabs.size })
    val scope = rememberCoroutineScope()
    val selectedTab = tabs[pagerState.currentPage]

    Column(modifier = modifier.fillMaxSize()) {
        StatsTabBar(
            selected = selectedTab,
            onSelect = { tab -> scope.launch { pagerState.animateScrollToPage(tab.ordinal) } }
        )
        HorizontalPager(state = pagerState, modifier = Modifier.weight(1f)) { page ->
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(bottom = 56.dp)
            ) {
                when (tabs[page]) {
                    StatsTab.SNAPSHOT -> snapshotTab(state, today, weekNum, weekLabel, weekCurrent, weekPrev, weekSessions, c)
                    StatsTab.STRENGTH -> strengthTab(state, c)
                    StatsTab.VOLUME -> volumeTab(state, c)
                    StatsTab.BODY -> bodyTab(state, c)
                    StatsTab.TRENDS -> trendsTab(state, c)
                }
            }
        }
    }
}
