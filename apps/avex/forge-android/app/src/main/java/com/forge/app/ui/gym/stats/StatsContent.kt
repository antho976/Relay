package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.gym.session.SegmentRow
import com.forge.app.ui.common.statsEntrance
import com.forge.app.ui.gym.stats.state.StatsUiState
import com.forge.app.ui.theme.LocalForgeSettings

/**
 * The lenses the Stats page reads through — the session-detail metric-picker pattern applied to the
 * whole history. One lens at a time; each renders a single comparative, tappable read.
 */
enum class StatsLens(val label: String) {
    STRENGTH("Strength"),
    VOLUME("Volume"),
    EFFORT("Effort"),
    DAYS("Days")
}

/**
 * Gym → Stats, rebuilt 2026-07-01 as "session detail, wider window": ONE page mirroring the
 * per-session screen's skeleton — a hero header (THIS WEEK figures + weekly muscle map + readiness
 * line), the same [SegmentRow] lens pills, one interactive comparison list per lens (rows expand
 * inline), then the always-on consistency heatmap and Records. No sub-tabs. The "Stats" title comes
 * from the hosting [com.forge.app.ui.gym.train.DayListScreen] app bar.
 */
@Composable
fun StatsContent(
    modifier: Modifier = Modifier,
    /** Opens a gym session's detail (the same screen History uses) — from the day sheet. */
    onOpenSession: (Long) -> Unit = {},
    /** Opens a cardio session's detail — from the day sheet. */
    onOpenCardioSession: (Long) -> Unit = {},
    viewModel: StatsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val weightUnit = LocalForgeSettings.current.weightUnit
    val dayDetail by viewModel.dayDetail.collectAsStateWithLifecycle()
    // A record tap deep-links into the Strength lens with that lift's row already open. focusNonce
    // bumps on every such tap so re-tapping the SAME lift (a no-op assignment to focusLift) still
    // re-expands its row after a manual collapse.
    var focusLift by rememberSaveable { mutableStateOf<String?>(null) }
    var focusNonce by rememberSaveable { mutableStateOf(0) }

    // Reopen on the last lens the user viewed (reuses the old tab pref; unknown stored names — the
    // retired tab names — fall back to the default). Restore once, THEN start persisting changes.
    val lastLensName by viewModel.lastStatsTabName.collectAsStateWithLifecycle()
    var lens by rememberSaveable { mutableStateOf(StatsLens.STRENGTH) }
    var restoredLens by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(lastLensName) {
        if (!restoredLens && lastLensName != null) {
            StatsLens.entries.firstOrNull { it.name == lastLensName }?.let { lens = it }
            restoredLens = true
        }
    }
    LaunchedEffect(restoredLens) {
        if (restoredLens) snapshotFlow { lens }.collect { viewModel.saveStatsTab(it.name) }
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
                "Couldn't load your stats just now. Swipe away and back. If it keeps happening, restart the app.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.fillMaxWidth().padding(horizontal = STATS_GUTTER, vertical = 12.dp)
            )
        }

        // Everything scrolls together — the hero is part of the page, like the session screen.
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(top = 4.dp, bottom = 56.dp)
        ) {
            if (state.isLoading) {
                loadingSkeleton(c)
            } else {
                // Always draw the page at zero — hero (honest-zero figures + muscle silhouette), lens
                // pills, then each lens's own zero-state — so first-run Stats reads as data at zero,
                // never a single blank hint (§12). Per-lens empties still fall to one InlineEmptyHint.
                // ── Hero — THIS WEEK figures + weekly muscle map + readiness line ──
                item("hero") {
                    Column(
                        Modifier.fillMaxWidth().statsEntrance(0)
                            .padding(horizontal = STATS_GUTTER, vertical = 10.dp)
                    ) {
                        StatsHeroContent(state, weightUnit, c)
                    }
                }

                // ── Lens pills — the session screen's metric picker, page-wide ──
                item("lens") {
                    Column(
                        Modifier.fillMaxWidth().statsEntrance(1)
                            .padding(horizontal = STATS_GUTTER, vertical = 6.dp)
                    ) {
                        SegmentRow(
                            items = StatsLens.entries,
                            isSelected = { it == lens },
                            label = { it.label },
                            onSelect = { lens = it },
                            onBg = c.onBg, muted = c.muted, accent = c.accent, outline = c.outline
                        )
                    }
                }

                when (lens) {
                    StatsLens.STRENGTH -> strengthLens(
                        state, weightUnit, c, focusLift, focusNonce,
                        onOpenLift = { id -> focusLift = id; focusNonce++; lens = StatsLens.STRENGTH }
                    )
                    StatsLens.VOLUME -> volumeLens(state, weightUnit, c)
                    StatsLens.EFFORT -> effortLens(state, c)
                    StatsLens.DAYS -> daysLens(state, c, onDayTap = viewModel::openDay)
                }
            }
            item("bottom-gap") { Spacer(Modifier.height(24.dp)) }
        }
    }

    // "What did I do that day?" — opened from the consistency heatmap; rows drill into the same
    // detail screens History uses.
    dayDetail?.let { detail ->
        StatsDayDetailSheet(
            detail = detail,
            onOpenSession = { viewModel.closeDay(); onOpenSession(it) },
            onOpenCardio = { viewModel.closeDay(); onOpenCardioSession(it) },
            onDismiss = viewModel::closeDay
        )
    }
}

/** A lens with no data yet shows a single quiet hint instead of a blank list (once loading settles). */
private fun LazyListScope.tabEmpty(key: String, text: String, c: StatsColors) {
    item(key) {
        InlineEmptyHint(text, color = c.muted, modifier = Modifier.padding(horizontal = STATS_GUTTER, vertical = 16.dp))
    }
}

/** Placeholder sections on first open so the screen shows structure, never a blank list, while data loads. */
private fun LazyListScope.loadingSkeleton(c: StatsColors) {
    repeat(3) { i ->
        item("skeleton-$i") {
            Column(
                Modifier
                    .fillMaxWidth()
                    .statsEntrance(i)
            ) {
                Spacer(Modifier.height(if (i == 0) 168.dp else 116.dp))
            }
        }
    }
}

// ── Strength lens — one comparative, tappable list per lift ─────────────────────
private fun LazyListScope.strengthLens(
    state: StatsUiState,
    weightUnit: WeightUnit,
    c: StatsColors,
    focusLift: String?,
    focusNonce: Int,
    onOpenLift: (String) -> Unit
) {
    if (state.e1rmLifts.isEmpty()) {
        tabEmpty("strength-empty", "Log a few weighted sets and your strength reads start here.", c)
        return
    }
    item("str-lifts") {
        StatsCard(
            c, title = "Strength per lift",
            caption = "Estimated 1RM, ranked. Tap a lift for its trend, PRs and curve.",
            index = 2
        ) {
            E1rmComparisonList(state.e1rmLifts, state.recentPrs, state.strengthCurves, weightUnit, c, focusLift, focusNonce)
        }
    }
    // Relative strength — each lift as a multiple of bodyweight.
    item("str-standards") {
        StatsCard(c, title = "Where you stand", caption = "Each lift as a multiple of bodyweight.", index = 3) {
            StrengthStandardsContent(state.e1rmLifts, state.bodyweightPoints.lastOrNull()?.weightLb, state.userSex, c)
        }
    }
    // The hall of fame lives here — records ARE strength, and a tap expands that lift's row above.
    if (state.hallOfFame.isNotEmpty()) {
        item("str-records") {
            StatsCard(c, title = "Records", caption = "Your heaviest set on each lift. Tap one for its trend.", index = 4) {
                RecordsContent(state.hallOfFame, weightUnit, c, onOpenLift)
            }
        }
    }
}

// ── Days lens — did I show up, and what shape am I in over time ─────────────────
private fun LazyListScope.daysLens(state: StatsUiState, c: StatsColors, onDayTap: (java.time.LocalDate) -> Unit) {
    if (state.dailyActivity.isEmpty()) {
        tabEmpty("days-empty", "Log a session and your training days light up here.", c)
        return
    }
    item("days-consistency") {
        StatsCard(c, title = "Consistency", caption = "Every training day, lit by how much you did.", index = 2) {
            AdherenceContent(state.dailyActivity, c, onDayTap)
        }
    }
    // The prettiest chart, deliberately last — viz-only nerd candy, not a decision gate.
    if (state.dailyActivity.size >= 5) {
        item("days-banister") {
            StatsCard(c, title = "Fitness vs fatigue", caption = "Banister model · viz only.", index = 3) {
                BanisterContent(state.dailyActivity, c)
            }
        }
    }
}

// ── Volume lens — targets, trend, balance (the weekly map lives in the hero) ───
private fun LazyListScope.volumeLens(state: StatsUiState, weightUnit: WeightUnit, c: StatsColors) {
    // Empty only when there's no logged volume at all. plannedSetsByMuscle is always populated from the
    // program, so it must NOT gate the empty state — otherwise a week with nothing logged yet renders a
    // wall of "0/target" bars that reads as "failing every target" instead of "no data yet".
    if (state.weeklySetsByMuscle.isEmpty() && state.weeklyTonnage.isEmpty() && state.balanceRatios.isEmpty()) {
        tabEmpty("volume-empty", "Log a session to see your volume break down.", c)
        return
    }
    if (state.weeklySetsByMuscle.isNotEmpty()) {
        item("vol-muscles") {
            StatsCard(c, title = "Sets per muscle this week", caption = "Each track is that muscle's weekly target. Fill it and you're on plan.", index = 2) {
                SetsPerMuscleContent(state.weeklySetsByMuscle, state.plannedSetsByMuscle, c)
            }
        }
    }
    if (state.weeklyTonnage.size >= 2) {
        item("vol-tonnage") {
            StatsCard(c, title = "Weekly volume", caption = "Total tonnage per week. Is it holding?", index = 3) {
                TonnageTrendContent(state.weeklyTonnage, weightUnit, c)
            }
        }
    }
    // Only worth a card once at least one pair has enough sets for the ratio to mean anything —
    // otherwise the section is empty rows and noise percentages.
    if (state.balanceRatios.any { it.setsA + it.setsB >= MIN_BALANCE_SETS }) {
        item("vol-balance") {
            StatsCard(c, title = "Balance", caption = "Push/pull and quad/ham. Is the work even?", index = 4) {
                BalanceContent(state.balanceRatios, c)
            }
        }
    }
}

// ── Effort lens — how hard the work felt ────────────────────────────────────────
private fun LazyListScope.effortLens(state: StatsUiState, c: StatsColors) {
    if (state.rpeDistribution.isEmpty()) {
        tabEmpty("effort-empty", "Rate a few sets (RPE) and your effort picture fills in here.", c)
        return
    }
    item("effort-rpe") {
        StatsCard(c, title = "Effort (RPE)", caption = "Where your working sets actually land.", index = 2) {
            RpeHistogramContent(state.rpeDistribution, state.avgRpe, c)
        }
    }
}
