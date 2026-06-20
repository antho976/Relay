package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.animation.core.tween
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.WeekFields
import java.util.Locale
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.gym.stats.state.E1rmLift
import com.forge.app.ui.theme.LocalForgeSettings

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
    val weightConnected by viewModel.weightConnected.collectAsStateWithLifecycle()
    val bodyweightMessage by viewModel.bodyweightMessage.collectAsStateWithLifecycle()
    var showWeightSheet by remember { mutableStateOf(false) }
    var selectedLift by remember { mutableStateOf<E1rmLift?>(null) }

    // Re-read the coach portrait whenever the screen (re)appears, so a coach pass that ran in the
    // background while away isn't shown stale until process restart (it loads once at VM init too).
    LaunchedEffect(Unit) { viewModel.refreshCoachWatch() }

    val context = LocalContext.current
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
    val useKg = LocalForgeSettings.current.useKg
    // ARGB Int (widened to Long for the renderer) — NOT Color.value, whose ULong packs channels
    // differently and truncates to ~0 (black/transparent) when the renderer does accentArgb.toInt().
    val accentArgb = MaterialTheme.colorScheme.primary.toArgb().toLong()

    val weekCurrent = state.weekComparison?.current
    val weekPrev = state.weekComparison?.previous
    val weekSessions = weekCurrent?.sessions ?: 0

    // Tap-only sub-tabs: the hub-level HorizontalPager owns left/right swipe now, so a nested pager
    // here would fight it (see HubScreen). Content crossfades between tabs instead of sliding.
    val tabs = StatsTab.entries
    var selectedIndex by rememberSaveable { mutableStateOf(0) }
    val selectedTab = tabs[selectedIndex]

    // Deep-link to the last sub-tab the user viewed (S4): restore once the stored value loads, THEN
    // start persisting changes — ordered so the initial tab-0 emission can't clobber the save.
    val lastTab by viewModel.lastStatsTab.collectAsStateWithLifecycle()
    var restoredTab by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(lastTab) {
        if (!restoredTab && lastTab >= 0) {
            if (lastTab in tabs.indices) selectedIndex = lastTab
            restoredTab = true
        }
    }
    LaunchedEffect(restoredTab) {
        if (restoredTab) snapshotFlow { selectedIndex }.collect { viewModel.saveStatsTab(it) }
    }

    Column(modifier = modifier.fillMaxSize()) {
        if (state.loadError) {
            Text(
                "Couldn't load your stats just now. Go back and reopen this tab — if it keeps happening, restart the app.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 12.dp)
            )
        }
        StatsTabBar(
            selected = selectedTab,
            onSelect = { tab -> selectedIndex = tab.ordinal }
        )
        AnimatedContent(
            targetState = selectedTab,
            modifier = Modifier.weight(1f),
            transitionSpec = { fadeIn(tween(200)) togetherWith fadeOut(tween(200)) },
            label = "stats-subtab"
        ) { tab ->
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(bottom = 56.dp)
            ) {
                when (tab) {
                    StatsTab.SNAPSHOT -> snapshotTab(state, today, weekNum, weekLabel, weekCurrent, weekPrev, weekSessions, c, onOpenNotes)
                    StatsTab.STRENGTH -> strengthTab(
                        state = state,
                        c = c,
                        onLiftClick = { selectedLift = it },
                        onShareTopLift = {
                            state.e1rmLifts.firstOrNull()?.let { top ->
                                val value = toDisplayWeight(top.currentE1rm, useKg).toInt().toString()
                                val unit = unitLabel(useKg).uppercase()
                                val rate = top.monthlyPct?.let { p ->
                                    "${if (p >= 0) "+" else ""}${"%.1f".format(p)}% / mo"
                                }
                                val uri = StatMetricCardRenderer.render(
                                    context = context,
                                    metricLabel = "ESTIMATED 1RM",
                                    liftName = top.exerciseName,
                                    heroValue = value,
                                    heroUnit = unit,
                                    rateLine = rate,
                                    accentArgb = accentArgb
                                )
                                uri?.let { StatMetricCardRenderer.share(context, it) }
                            }
                        }
                    )
                    StatsTab.VOLUME -> volumeTab(state, c)
                    StatsTab.BODY -> bodyTab(state, c, onLogWeight = {
                        // Fresh sheet: drop any prior "Saved."/import line and re-check HC permission so
                        // a grant made in Settings since this screen opened surfaces the import option.
                        viewModel.clearBodyweightMessage()
                        viewModel.refreshWeightConnected()
                        showWeightSheet = true
                    })
                    StatsTab.TRENDS -> trendsTab(state, c)
                }
            }
        }
    }

    selectedLift?.let { lift ->
        StatsLiftDetailSheet(
            lift = lift,
            hallRecord = state.hallOfFame.find { it.exerciseId == lift.exerciseId },
            recentHistory = state.exerciseHistory[lift.exerciseId].orEmpty(),
            onDismiss = { selectedLift = null }
        )
    }

    if (showWeightSheet) {
        BodyweightLogSheet(
            latestLb = state.bodyweightPoints.lastOrNull()?.weightLb,
            canImport = weightConnected,
            message = bodyweightMessage,
            onSave = { lb ->
                viewModel.logBodyweight(lb)
                showWeightSheet = false
            },
            onImport = { viewModel.importBodyweight() },  // stays open so the result line shows
            onDismiss = {
                showWeightSheet = false
                viewModel.clearBodyweightMessage()
            }
        )
    }
}
