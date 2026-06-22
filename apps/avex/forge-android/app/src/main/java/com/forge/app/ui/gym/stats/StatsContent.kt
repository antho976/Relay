package com.forge.app.ui.gym.stats

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
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
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.gym.stats.state.StatsUiState
import com.forge.app.ui.theme.LocalForgeSettings

/**
 * Gym → Stats. The charts that earn their place by driving decisions, grouped into tap-only sub-tabs
 * ([StatsTab]) so each is a short list rather than one long scroll.
 *
 * Rebuilt ground-up, tier by tier; each chart is a self-contained [LazyListScope] section so tiers can
 * be reordered or moved between tabs without touching the scaffold. The "Stats" title comes from the
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
    val lastTab by viewModel.lastStatsTab.collectAsStateWithLifecycle()
    var selectedTab by rememberSaveable { mutableStateOf(StatsTab.STRENGTH) }
    var restoredTab by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(lastTab) {
        if (!restoredTab && lastTab >= 0) {
            StatsTab.entries.getOrNull(lastTab)?.let { selectedTab = it }
            restoredTab = true
        }
    }
    LaunchedEffect(restoredTab) {
        if (restoredTab) snapshotFlow { selectedTab }.collect { viewModel.saveStatsTab(it.ordinal) }
    }

    val c = StatsColors(
        onBg = MaterialTheme.colorScheme.onBackground,
        muted = MaterialTheme.colorScheme.onSurfaceVariant,
        accent = MaterialTheme.colorScheme.primary,
        outline = MaterialTheme.colorScheme.outline
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
                when (tab) {
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
private fun LazyListScope.tabEmpty(key: String, loading: Boolean, text: String, c: StatsColors) {
    if (!loading) item(key) {
        InlineEmptyHint(text, color = c.muted, modifier = Modifier.padding(horizontal = STATS_GUTTER, vertical = 16.dp))
    }
}

private fun LazyListScope.sectionGap(key: String) = item(key) { Spacer(Modifier.height(20.dp)) }

// ── Strength tab — e1RM over time + PRs + load-rep curve ────────────────────────
private fun LazyListScope.strengthTab(state: StatsUiState, useKg: Boolean, c: StatsColors) {
    if (state.e1rmLifts.isEmpty() && state.recentPrs.isEmpty() && state.strengthCurves.isEmpty()) {
        tabEmpty("strength-empty", state.isLoading,
            "Log a few weighted sets and your strength charts start here.", c)
        return
    }
    if (state.e1rmLifts.isNotEmpty()) {
        statsSection("strength-title", "Strength over time", c)
        statsCaption("strength-caption", "Estimated 1RM per lift — tap a lift to see its trend.", c)
        e1rmLedger(state.e1rmLifts, useKg, c)
    }
    if (state.recentPrs.isNotEmpty()) {
        sectionGap("pr-gap")
        statsSection("pr-title", "Personal records", c)
        statsCaption("pr-caption", "Every PR on a time axis, recent ones spelled out.", c)
        prTimelineSection(state.recentPrs, useKg, c)
    }
    if (state.strengthCurves.isNotEmpty()) {
        sectionGap("curve-gap")
        statsSection("curve-title", "Strength curve", c)
        statsCaption("curve-caption", "Every set as weight × reps, with the fitted curve and projected 1RM.", c)
        strengthCurveSection(state.strengthCurves, useKg, c)
    }
}

// ── Volume tab — sets per muscle + balance + the muscle heatmap ─────────────────
private fun LazyListScope.volumeTab(state: StatsUiState, useKg: Boolean, c: StatsColors) {
    // Empty only when there's no logged volume at all. plannedSetsByMuscle is always populated from the
    // program, so it must NOT gate the empty state — otherwise a week with nothing logged yet renders a
    // wall of "0/target" bars that reads as "failing every target" instead of "no data yet".
    if (state.weeklySetsByMuscle.isEmpty() && state.weeklyTonnage.isEmpty() && state.balanceRatios.isEmpty()) {
        tabEmpty("volume-empty", state.isLoading,
            "Log a session to see your volume break down.", c)
        return
    }
    // Sets-per-muscle and the heatmap need actual logged sets this week; without them, skip rather than
    // show every muscle at 0/target.
    if (state.weeklySetsByMuscle.isNotEmpty()) {
        statsSection("vol-title", "Sets per muscle this week", c)
        statsCaption("vol-caption", "Bars vs each muscle's target tick — am I doing enough?", c)
        setsPerMuscleSection(state.weeklySetsByMuscle, state.plannedSetsByMuscle, c)
    }
    if (state.weeklyTonnage.size >= 2) {
        sectionGap("tonnage-gap")
        statsSection("tonnage-title", "Weekly volume", c)
        statsCaption("tonnage-caption", "Total tonnage per week — is it holding?", c)
        tonnageTrendSection(state.weeklyTonnage, useKg, c)
    }
    if (state.balanceRatios.isNotEmpty()) {
        sectionGap("bal-gap")
        statsSection("bal-title", "Balance", c)
        statsCaption("bal-caption", "Push/pull and quad/ham — is the work even?", c)
        balanceSection(state.balanceRatios, c)
    }
    if (state.weeklySetsByMuscle.isNotEmpty()) {
        sectionGap("map-gap")
        statsSection("map-title", "Where the work landed", c)
        statsCaption("map-caption", "Each muscle tinted by this week's sets — faint = neglected.", c)
        muscleMapSection(state.weeklySetsByMuscle, state.plannedSetsByMuscle, c)
    }
}

// ── Body tab — bodyweight, relative strength, readiness ─────────────────────────
private fun LazyListScope.bodyTab(state: StatsUiState, useKg: Boolean, c: StatsColors, onLogWeight: () -> Unit) {
    // Always present — the quick-log is the only manual bodyweight entry point after onboarding, so it
    // must show even before there's any data (the early-return below would otherwise hide it).
    item("bw-log-action") { BodyweightLogButton(c, onLogWeight) }
    if (state.bodyweightPoints.isEmpty() && state.e1rmLifts.isEmpty() && state.readinessPulse == null) {
        tabEmpty("body-empty", state.isLoading,
            "Log your bodyweight and a few sessions to see where you stand.", c)
        return
    }
    if (state.bodyweightPoints.isNotEmpty()) {
        statsSection("bw-title", "Bodyweight", c)
        statsCaption("bw-caption", "A moving average over your raw weigh-ins.", c)
        bodyweightSection(state.bodyweightPoints, useKg, c)
    }
    if (state.e1rmLifts.isNotEmpty()) {
        sectionGap("std-gap")
        statsSection("std-title", "Where you stand", c)
        statsCaption("std-caption", "Each lift as a multiple of bodyweight.", c)
        strengthStandardsSection(state.e1rmLifts, state.bodyweightPoints.lastOrNull()?.weightLb, state.userSex, c)
    }
    if (state.readinessPulse != null && state.readinessThreshold != null) {
        sectionGap("ready-gap")
        statsSection("ready-title", "Readiness", c)
        statsCaption("ready-caption", "Fatigue against your learned deload line.", c)
        readinessSection(state.readinessPulse, state.readinessThreshold, c)
    }
}

// ── Trends tab — consistency + the subordinate "for interest" charts ────────────
private fun LazyListScope.trendsTab(state: StatsUiState, c: StatsColors) {
    val hasAny = state.dailyActivity.isNotEmpty() || state.rpeDistribution.isNotEmpty() ||
        state.trainingTimes != null || state.prsByDayOfWeek.any { it > 0 }
    if (!hasAny) {
        tabEmpty("trends-empty", state.isLoading, "A few logged sessions and your trends fill in here.", c)
        return
    }
    if (state.dailyActivity.isNotEmpty()) {
        statsSection("adh-title", "Consistency", c)
        statsCaption("adh-caption", "Every training day, lit by how much you did.", c)
        adherenceSection(state.dailyActivity, c)
    }
    if (state.rpeDistribution.isNotEmpty()) {
        sectionGap("rpe-gap")
        statsSection("rpe-title", "Effort (RPE)", c)
        rpeHistogramSection(state.rpeDistribution, state.avgRpe, c)
    }
    if (state.trainingTimes != null || state.prsByDayOfWeek.any { it > 0 }) {
        sectionGap("patterns-gap")
        statsSection("patterns-title", "Patterns", c)
        trainingPatternsSection(state.trainingTimes, state.prsByDayOfWeek, c)
    }
    if (state.dailyActivity.size >= 5) {
        sectionGap("banister-gap")
        statsSection("banister-title", "Fitness vs fatigue", c)
        statsCaption("banister-caption", "Banister model — viz only.", c)
        banisterSection(state.dailyActivity, c)
    }
}
