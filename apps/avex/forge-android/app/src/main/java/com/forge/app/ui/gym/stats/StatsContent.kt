package com.forge.app.ui.gym.stats

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.gym.stats.components.statsEntrance
import com.forge.app.ui.gym.stats.state.StatsUiState
import com.forge.app.ui.theme.LocalForgeSettings

/**
 * Gym → Stats. The charts that earn their place by driving decisions, grouped into tap-only sub-tabs
 * ([StatsTab]) so each is a short list of quiet [StatsCard]s rather than one long flat scroll.
 *
 * OVERVIEW is the landing tab: a populated headline read (top lift + lifetime tiles + this week +
 * records) so the screen is never just-the-tab-pills on first open. The "Stats" title comes from the
 * hosting [com.forge.app.ui.gym.train.DayListScreen] app bar.
 */
@Composable
fun StatsContent(
    modifier: Modifier = Modifier,
    viewModel: StatsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val useKg = LocalForgeSettings.current.useKg
    val weightConnected by viewModel.weightConnected.collectAsStateWithLifecycle()
    val bodyweightMessage by viewModel.bodyweightMessage.collectAsStateWithLifecycle()
    var showWeightSheet by remember { mutableStateOf(false) }

    // Deep-link to the last sub-tab the user viewed (S4): restore once the stored value loads, THEN
    // start persisting changes — ordered so the initial default emission can't clobber the save.
    val lastTabName by viewModel.lastStatsTabName.collectAsStateWithLifecycle()
    var selectedTab by rememberSaveable { mutableStateOf(StatsTab.OVERVIEW) }
    var restoredTab by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(lastTabName) {
        // Match by enum NAME, not ordinal, so inserting/reordering tabs never restores the wrong one.
        if (!restoredTab && lastTabName != null) {
            StatsTab.entries.firstOrNull { it.name == lastTabName }?.let { selectedTab = it }
            restoredTab = true
        }
    }
    LaunchedEffect(restoredTab) {
        if (restoredTab) snapshotFlow { selectedTab }.collect { viewModel.saveStatsTab(it.name) }
    }

    val c = StatsColors(
        onBg = MaterialTheme.colorScheme.onBackground,
        muted = MaterialTheme.colorScheme.onSurfaceVariant,
        accent = MaterialTheme.colorScheme.primary,
        outline = MaterialTheme.colorScheme.outline,
        surface = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
    )

    Column(modifier.fillMaxSize()) {
        if (state.loadError) {
            Text(
                "Couldn't load your stats just now. Swipe away and back — if it keeps happening, restart the app.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.fillMaxWidth().padding(horizontal = STATS_GUTTER, vertical = 12.dp)
            )
        }

        StatsTabRow(selected = selectedTab, onSelect = { selectedTab = it }, c = c)

        AnimatedContent(
            targetState = selectedTab,
            transitionSpec = { fadeIn(tween(200)) togetherWith fadeOut(tween(200)) },
            modifier = Modifier.weight(1f),
            label = "stats-tab"
        ) { tab ->
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(top = 12.dp, bottom = 56.dp)
            ) {
                if (state.isLoading) {
                    loadingSkeleton(c)
                } else when (tab) {
                    StatsTab.OVERVIEW -> overviewTab(state, useKg, c)
                    StatsTab.STRENGTH -> strengthTab(state, useKg, c)
                    StatsTab.VOLUME -> volumeTab(state, useKg, c)
                    StatsTab.BODY -> bodyTab(state, useKg, c, onLogWeight = {
                        // Fresh sheet: drop any prior result line and re-check HC permission so a grant
                        // made in Settings since this screen opened surfaces the import option.
                        viewModel.clearBodyweightMessage()
                        viewModel.refreshWeightConnected()
                        showWeightSheet = true
                    })
                    StatsTab.TRENDS -> trendsTab(state, c)
                }
                item("bottom-gap") { Spacer(Modifier.height(24.dp)) }
            }
        }
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

/** A tab with no data yet shows a single quiet hint instead of a blank list (once loading settles). */
private fun LazyListScope.tabEmpty(key: String, text: String, c: StatsColors) {
    item(key) {
        InlineEmptyHint(text, color = c.muted, modifier = Modifier.padding(horizontal = STATS_GUTTER, vertical = 16.dp))
    }
}

/** Placeholder cards on first open so the screen shows structure, never a blank list, while data loads. */
private fun LazyListScope.loadingSkeleton(c: StatsColors) {
    repeat(3) { i ->
        item("skeleton-$i") {
            Column(
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = STATS_GUTTER, vertical = STATS_CARD_GAP)
                    .statsEntrance(i)
                    .clip(RoundedCornerShape(STATS_CARD_RADIUS))
                    .background(c.surface)
                    .height(if (i == 0) 168.dp else 116.dp)
            ) {}
        }
    }
}

// ── Overview tab — the populated landing read ───────────────────────────────────
private fun LazyListScope.overviewTab(state: StatsUiState, useKg: Boolean, c: StatsColors) {
    val hero = state.e1rmLifts.firstOrNull()
    val nothing = hero == null && state.lifetime == null && state.hallOfFame.isEmpty() && state.weekComparison == null
    if (nothing) {
        tabEmpty("overview-empty", "Log a session and your overview fills in here.", c)
        return
    }
    if (hero != null) {
        item("ov-hero") { StatsCard(c, index = 0) { OverviewHeroContent(hero, useKg, c) } }
    }
    state.lifetime?.let { lifetime ->
        item("ov-life") {
            StatsCard(c, title = "At a glance", index = 1) {
                LifetimeTilesContent(lifetime, state.hallOfFame.size, useKg, c)
            }
        }
    }
    state.weekComparison?.let { cmp ->
        item("ov-week") {
            StatsCard(c, title = "This week", caption = "vs last week", index = 2) {
                WeekComparisonContent(cmp, useKg, c)
            }
        }
    }
    if (state.hallOfFame.isNotEmpty()) {
        item("ov-records") {
            StatsCard(c, title = "Records", caption = "Your heaviest set on each lift.", index = 3) {
                RecordsContent(state.hallOfFame, useKg, c)
            }
        }
    }
}

// ── Strength tab — e1RM over time + PRs + load-rep curve ────────────────────────
private fun LazyListScope.strengthTab(state: StatsUiState, useKg: Boolean, c: StatsColors) {
    if (state.e1rmLifts.isEmpty() && state.recentPrs.isEmpty() && state.strengthCurves.isEmpty()) {
        tabEmpty("strength-empty", "Log a few weighted sets and your strength charts start here.", c)
        return
    }
    if (state.e1rmLifts.isNotEmpty()) {
        item("strength-card") {
            StatsCard(c, title = "Strength over time", caption = "Estimated 1RM per lift — tap a lift to see its trend.", index = 0) {
                E1rmLedgerContent(state.e1rmLifts, useKg, c)
            }
        }
    }
    if (state.recentPrs.isNotEmpty()) {
        item("pr-card") {
            StatsCard(c, title = "Personal records", caption = "Every PR on a time axis, recent ones spelled out.", index = 1) {
                PrTimelineContent(state.recentPrs, useKg, c)
            }
        }
    }
    if (state.strengthCurves.isNotEmpty()) {
        item("curve-card") {
            StatsCard(c, title = "Strength curve", caption = "Every set as weight × reps, with the fitted curve and projected 1RM.", index = 2) {
                StrengthCurveContent(state.strengthCurves, useKg, c)
            }
        }
    }
}

// ── Volume tab — sets per muscle + balance + the muscle heatmap ─────────────────
private fun LazyListScope.volumeTab(state: StatsUiState, useKg: Boolean, c: StatsColors) {
    // Empty only when there's no logged volume at all. plannedSetsByMuscle is always populated from the
    // program, so it must NOT gate the empty state — otherwise a week with nothing logged yet renders a
    // wall of "0/target" bars that reads as "failing every target" instead of "no data yet".
    if (state.weeklySetsByMuscle.isEmpty() && state.weeklyTonnage.isEmpty() && state.balanceRatios.isEmpty()) {
        tabEmpty("volume-empty", "Log a session to see your volume break down.", c)
        return
    }
    if (state.weeklySetsByMuscle.isNotEmpty()) {
        item("vol-card") {
            StatsCard(c, title = "Sets per muscle this week", caption = "Bars vs each muscle's target tick — am I doing enough?", index = 0) {
                SetsPerMuscleContent(state.weeklySetsByMuscle, state.plannedSetsByMuscle, c)
            }
        }
    }
    if (state.weeklyTonnage.size >= 2) {
        item("tonnage-card") {
            StatsCard(c, title = "Weekly volume", caption = "Total tonnage per week — is it holding?", index = 1) {
                TonnageTrendContent(state.weeklyTonnage, useKg, c)
            }
        }
    }
    if (state.balanceRatios.isNotEmpty()) {
        item("bal-card") {
            StatsCard(c, title = "Balance", caption = "Push/pull and quad/ham — is the work even?", index = 2) {
                BalanceContent(state.balanceRatios, c)
            }
        }
    }
    if (state.weeklySetsByMuscle.isNotEmpty()) {
        item("map-card") {
            StatsCard(c, title = "Where the work landed", caption = "Each muscle tinted by this week's sets — faint = neglected.", index = 3) {
                MuscleMapContent(state.weeklySetsByMuscle, state.plannedSetsByMuscle, c)
            }
        }
    }
}

// ── Body tab — bodyweight, relative strength, readiness ─────────────────────────
private fun LazyListScope.bodyTab(state: StatsUiState, useKg: Boolean, c: StatsColors, onLogWeight: () -> Unit) {
    // Always present — the quick-log is the only manual bodyweight entry point after onboarding.
    item("bw-log-action") { StatsCard(c, index = 0) { BodyweightLogButton(c, onLogWeight) } }
    if (state.bodyweightPoints.isEmpty() && state.e1rmLifts.isEmpty() && state.readinessPulse == null) {
        tabEmpty("body-empty", "Log your bodyweight and a few sessions to see where you stand.", c)
        return
    }
    if (state.bodyweightPoints.isNotEmpty()) {
        item("bw-card") {
            StatsCard(c, title = "Bodyweight", caption = "A moving average over your raw weigh-ins.", index = 1) {
                BodyweightContent(state.bodyweightPoints, useKg, c)
            }
        }
    }
    if (state.e1rmLifts.isNotEmpty()) {
        item("std-card") {
            StatsCard(c, title = "Where you stand", caption = "Each lift as a multiple of bodyweight.", index = 2) {
                StrengthStandardsContent(state.e1rmLifts, state.bodyweightPoints.lastOrNull()?.weightLb, state.userSex, c)
            }
        }
    }
    if (state.readinessPulse != null && state.readinessThreshold != null) {
        item("ready-card") {
            StatsCard(c, title = "Readiness", caption = "Fatigue against your learned deload line.", index = 3) {
                ReadinessContent(state.readinessPulse, state.readinessThreshold, c)
            }
        }
    }
}

// ── Trends tab — consistency + the subordinate "for interest" charts ────────────
private fun LazyListScope.trendsTab(state: StatsUiState, c: StatsColors) {
    val hasAny = state.dailyActivity.isNotEmpty() || state.rpeDistribution.isNotEmpty() ||
        state.trainingTimes != null || state.prsByDayOfWeek.any { it > 0 }
    if (!hasAny) {
        tabEmpty("trends-empty", "A few logged sessions and your trends fill in here.", c)
        return
    }
    if (state.dailyActivity.isNotEmpty()) {
        item("adh-card") {
            StatsCard(c, title = "Consistency", caption = "Every training day, lit by how much you did.", index = 0) {
                AdherenceContent(state.dailyActivity, c)
            }
        }
    }
    if (state.rpeDistribution.isNotEmpty()) {
        item("rpe-card") {
            StatsCard(c, title = "Effort (RPE)", index = 1) {
                RpeHistogramContent(state.rpeDistribution, state.avgRpe, c)
            }
        }
    }
    if (state.trainingTimes != null || state.prsByDayOfWeek.any { it > 0 }) {
        item("patterns-card") {
            StatsCard(c, title = "Patterns", index = 2) {
                TrainingPatternsContent(state.trainingTimes, state.prsByDayOfWeek, c)
            }
        }
    }
    if (state.dailyActivity.size >= 5) {
        item("banister-card") {
            StatsCard(c, title = "Fitness vs fatigue", caption = "Banister model — viz only.", index = 3) {
                BanisterContent(state.dailyActivity, c)
            }
        }
    }
}
