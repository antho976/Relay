package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.gym.stats.components.CalendarCard
import com.forge.app.ui.gym.stats.components.DayTypeBestVsAvgCard
import com.forge.app.ui.gym.stats.components.EffortDistributionCard
import com.forge.app.ui.gym.stats.components.ExerciseFrequencyCard
import com.forge.app.ui.gym.stats.components.ExerciseHistorySheet
import com.forge.app.ui.gym.stats.components.FrequencyHeatmap
import com.forge.app.ui.gym.stats.components.HallOfFameSection
import com.forge.app.ui.gym.stats.components.PrDayOfWeekCard
import com.forge.app.ui.gym.stats.components.PrTimelineList
import com.forge.app.ui.gym.stats.components.StrengthCurveCard
import com.forge.app.ui.gym.stats.components.TimeToPrCard
import com.forge.app.ui.gym.stats.components.TotalsRow
import com.forge.app.ui.gym.stats.components.VolumeByMuscleChart
import com.forge.app.ui.gym.stats.components.DayTypeBreakdownCard
import com.forge.app.ui.gym.stats.components.EffortOverTimeCard
import com.forge.app.ui.gym.stats.components.ExerciseVolumeChart
import com.forge.app.ui.gym.stats.components.PrTimelineSheet
import com.forge.app.ui.gym.stats.components.StrengthOverlayCard
import com.forge.app.ui.gym.stats.components.InsightsCard
import com.forge.app.ui.gym.stats.components.LifetimeMetricsCard
import com.forge.app.ui.gym.stats.components.ExerciseYoYCard
import com.forge.app.ui.gym.stats.components.PeriodComparisonCard
import com.forge.app.ui.gym.stats.components.PrClusteringCard
import com.forge.app.ui.gym.stats.components.StrengthRadarCard
import com.forge.app.ui.gym.stats.components.VolumeDeloadCard
import com.forge.app.ui.gym.stats.components.VolumeDonutCard
import com.forge.app.ui.gym.stats.state.PrRecord

/**
 * The Stats subtab content. Lives inside DayListScreen's tab pager; no top bar of its
 * own — the parent screen owns navigation chrome.
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
    var historySheet by remember { mutableStateOf<PrRecord?>(null) }
    var prTimelineSheet by remember { mutableStateOf<PrRecord?>(null) }

    LazyColumn(
        modifier = modifier,
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        if (state.totals.workouts > 0) {
            item("hero") {
                Text(
                    "${state.totals.workouts} sessions.",
                    style = MaterialTheme.typography.headlineLarge,
                    color = MaterialTheme.colorScheme.onBackground
                )
                Spacer(Modifier.height(2.dp))
                Text(
                    "${state.totals.prs} PRs · ${state.totals.exercisesLogged} exercises logged.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.height(16.dp))
            }
        }
        if (state.totals.workouts == 0 && !state.isLoading) {
            item("empty") {
                com.forge.app.ui.common.EmptyState(
                    emoji = "📊",
                    title = "No data yet.",
                    subtitle = "Log your first workout to see stats here."
                )
            }
        }
        item("history-btn") {
            androidx.compose.material3.TextButton(
                onClick = onOpenHistory,
                modifier = Modifier.fillMaxWidth()
            ) { androidx.compose.material3.Text("View session history →") }
        }
        item("recap-btn") {
            androidx.compose.material3.TextButton(
                onClick = onOpenRecap,
                modifier = Modifier.fillMaxWidth()
            ) { androidx.compose.material3.Text("View monthly & yearly recap →") }
        }
        item("totals") { TotalsRow(totals = state.totals) }
        item("heatmap") { FrequencyHeatmap(cells = state.heatmap) }
        item("volume") { VolumeByMuscleChart(rows = state.volumeByMuscle) }
        item("curve") { StrengthCurveCard(curve = state.strengthCurve) }
        item("pr-timeline") { PrTimelineList(entries = state.recentPrs) }
        state.monthCalendar?.let { cal ->
            item("calendar") {
                CalendarCard(
                    data = cal,
                    onMarkRestDay = { dateKey, type -> viewModel.markRestDay(dateKey, type) },
                    onClearRestDay = { dateKey -> viewModel.clearRestDay(dateKey) }
                )
            }
        }
        item("notes-search-btn") {
            androidx.compose.material3.TextButton(
                onClick = onOpenNotes,
                modifier = Modifier.fillMaxWidth()
            ) { androidx.compose.material3.Text("Search notes →") }
        }
        item("records") {
            HallOfFameSection(
                records = state.hallOfFame,
                onRecordClick = { historySheet = it }
            )
        }
        if (state.effortDistribution.isNotEmpty()) {
            item("effort-dist") { EffortDistributionCard(weeks = state.effortDistribution) }
        }
        if (state.exerciseFrequency.isNotEmpty()) {
            item("ex-freq") { ExerciseFrequencyCard(data = state.exerciseFrequency) }
        }
        if (state.timeToPr.isNotEmpty()) {
            item("time-to-pr") { TimeToPrCard(data = state.timeToPr) }
        }
        if (state.prsByDayOfWeek.any { it > 0 }) {
            item("pr-dow") { PrDayOfWeekCard(counts = state.prsByDayOfWeek) }
        }
        // Strength overlay: top 2 exercises by frequency
        val topTwo = state.exerciseFrequency.take(2)
        if (topTwo.size == 2) {
            val h1 = topTwo[0].exerciseName to (state.exerciseHistory[topTwo[0].exerciseId] ?: emptyList())
            val h2 = topTwo[1].exerciseName to (state.exerciseHistory[topTwo[1].exerciseId] ?: emptyList())
            if (h1.second.size >= 2 || h2.second.size >= 2) {
                item("str-overlay") { StrengthOverlayCard(history1 = h1, history2 = h2) }
            }
        }
        if (state.moodOverTime.size >= 3) {
            item("effort-time") { EffortOverTimeCard(moodData = state.moodOverTime) }
        }
        if (state.volumeByMuscle.isNotEmpty()) {
            item("vol-donut") { VolumeDonutCard(rows = state.volumeByMuscle) }
        }
        if (state.volumeDeloadTrend.size >= 2) {
            item("vol-deload") { VolumeDeloadCard(points = state.volumeDeloadTrend) }
        }
        if (state.dayTypeBestVsAvg.isNotEmpty()) {
            item("day-best-avg") { DayTypeBestVsAvgCard(data = state.dayTypeBestVsAvg) }
        }
        state.weekComparison?.let { cmp ->
            item("week-cmp") { PeriodComparisonCard(comparison = cmp) }
        }
        state.monthComparison?.let { cmp ->
            item("month-cmp") { PeriodComparisonCard(comparison = cmp) }
        }
        if (state.exerciseYoY.isNotEmpty()) {
            item("yoy") { ExerciseYoYCard(data = state.exerciseYoY) }
        }
        state.lifetimeMetrics?.let { lm ->
            if (lm.totalSessions > 0) item("lifetime") { LifetimeMetricsCard(metrics = lm) }
        }
        if (state.insights.isNotEmpty()) {
            item("insights") { InsightsCard(flags = state.insights) }
        }
        if (state.dayTypeBreakdown.isNotEmpty()) {
            item("day-breakdown") { DayTypeBreakdownCard(data = state.dayTypeBreakdown) }
        }
        if (state.compoundMaxes.isNotEmpty()) {
            item("radar") { StrengthRadarCard(compoundMaxes = state.compoundMaxes) }
        }
        if (state.prSessionTimestamps.size >= 3) {
            item("pr-cluster") { PrClusteringCard(prTimestamps = state.prSessionTimestamps) }
        }
        // Top 3 exercises by frequency get a volume-over-time chart
        state.exerciseFrequency.take(3).forEach { ef ->
            val vols = state.exerciseVolumeHistory[ef.exerciseId] ?: emptyList()
            if (vols.size >= 2) {
                item("vol-${ef.exerciseId}") {
                    ExerciseVolumeChart(exerciseId = ef.exerciseId, points = vols)
                }
            }
        }
    }

    historySheet?.let { record ->
        ExerciseHistorySheet(
            exerciseName = record.exerciseName,
            history = state.exerciseHistory[record.exerciseId] ?: emptyList(),
            volumeHistory = state.exerciseVolumeHistory[record.exerciseId] ?: emptyList(),
            onOpenTimeline = { prTimelineSheet = record },
            onDismiss = { historySheet = null }
        )
    }
    prTimelineSheet?.let { record ->
        PrTimelineSheet(
            exerciseName = record.exerciseName,
            history = state.exerciseHistory[record.exerciseId] ?: emptyList(),
            onDismiss = { prTimelineSheet = null }
        )
    }
}
