package com.forge.app.ui.gym.stats

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.components.DayTypeBestVsAvgCard
import com.forge.app.ui.gym.stats.components.EffortOverTimeCard
import com.forge.app.ui.gym.stats.components.HallOfFameRow
import com.forge.app.ui.gym.stats.components.PrClusteringCard
import com.forge.app.ui.gym.stats.components.PrDayOfWeekCard
import com.forge.app.ui.gym.stats.components.PrEntryRow
import com.forge.app.ui.gym.stats.components.RhythmRow
import com.forge.app.ui.gym.stats.components.StrengthOverlayCard
import com.forge.app.ui.gym.stats.components.StrengthRadarCard
import com.forge.app.ui.gym.stats.components.TimeToPrCard
import com.forge.app.ui.gym.stats.components.VolumeDeloadCard
import com.forge.app.ui.gym.stats.components.VolumeDonutCard
import com.forge.app.ui.gym.stats.components.WeekDayRow
import com.forge.app.ui.gym.stats.components.emptyWeekActivity
import com.forge.app.ui.gym.stats.state.PeriodStats
import com.forge.app.ui.gym.stats.state.StatsUiState
import java.time.LocalDate

enum class StatsTab(val label: String) {
    SNAPSHOT("Snapshot"),
    STRENGTH("Strength"),
    VOLUME("Volume"),
    BODY("Body"),
    TRENDS("Trends")
}

/** Bundles the theme colors so the tab builders don't take a dozen color params each. */
data class StatsColors(val onBg: Color, val muted: Color, val accent: Color, val outline: Color)

@Composable
fun StatsTabBar(selected: StatsTab, onSelect: (StatsTab) -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 8.dp)
    ) {
        StatsTab.entries.forEach { tab ->
            val isSel = tab == selected
            Box(
                modifier = Modifier
                    .padding(end = 8.dp)
                    .clickable { onSelect(tab) }
                    .then(
                        if (isSel) Modifier.background(onBg, RoundedCornerShape(50))
                        else Modifier.border(1.dp, outline.copy(alpha = 0.5f), RoundedCornerShape(50))
                    )
                    .padding(horizontal = 14.dp, vertical = 7.dp)
            ) {
                Text(
                    tab.label,
                    style = MaterialTheme.typography.labelMedium,
                    color = if (isSel) MaterialTheme.colorScheme.background else muted,
                    fontWeight = if (isSel) FontWeight.SemiBold else FontWeight.Normal
                )
            }
        }
    }
    HorizontalDivider(color = outline.copy(alpha = 0.2f))
}

private fun LazyListScope.sectionTitle(key: String, title: String, c: StatsColors) {
    item(key) {
        Text(
            title,
            style = MaterialTheme.typography.headlineSmall,
            color = c.onBg,
            fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(horizontal = 24.dp)
        )
        Spacer(Modifier.height(12.dp))
    }
}

/** Wrap a card-style analytics composable with the screen's horizontal padding. */
private fun LazyListScope.cardItem(key: String, content: @Composable () -> Unit) {
    item(key) {
        Box(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) { content() }
    }
}

// ─── Snapshot ─────────────────────────────────────────────────────────────────

internal fun LazyListScope.snapshotTab(
    state: StatsUiState,
    today: LocalDate,
    weekNum: Int,
    weekLabel: String,
    weekCurrent: PeriodStats?,
    weekPrev: PeriodStats?,
    weekSessions: Int,
    c: StatsColors
) {
    item("hero") {
        StatsHeroSection(
            weekNum = weekNum, weekLabel = weekLabel, weekSessions = weekSessions,
            weekCurrentVolumeLb = weekCurrent?.volumeLb, weekCurrentPrs = weekCurrent?.prs ?: 0,
            cardioMin = state.thisWeekCardioMin, onBg = c.onBg, muted = c.muted
        )
    }
    item("momentum") { MomentumGrid(weekCurrent, weekPrev, c.onBg, c.muted, c.outline) }
    item("highlights") { HighlightCards(state.consistencyStreakWeeks, state.progressiveOverloadPct, c.onBg, c.muted, c.outline) }
    item("rhythm") {
        Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
            Text("RHYTHM", style = MaterialTheme.typography.labelSmall, color = c.muted, fontSize = 9.sp)
            Spacer(Modifier.height(8.dp))
            RhythmRow(weekActivity = state.weekActivity, today = today, onBg = c.onBg, muted = c.muted, outline = c.outline)
            Spacer(Modifier.height(20.dp))
            HorizontalDivider(color = c.outline.copy(alpha = 0.25f))
            Spacer(Modifier.height(20.dp))
        }
    }
    sectionTitle("week-title", "What I did this week", c)
    items(state.weekActivity.ifEmpty { emptyWeekActivity() }, key = { "day-${it.dayOfWeek}" }) { row ->
        WeekDayRow(row = row, today = today, onBg = c.onBg, muted = c.muted, accent = c.accent,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 5.dp))
    }
    item("lifetime") { StatsLifetimeSection(lm = state.lifetimeMetrics, onBg = c.onBg, muted = c.muted, outline = c.outline) }
    if (state.insights.isNotEmpty()) {
        sectionTitle("insights-title", "Insights", c)
        items(state.insights, key = { "insight-${it.title}" }) { flag ->
            Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 6.dp)) {
                Text(flag.title, style = MaterialTheme.typography.labelSmall, color = c.muted, fontSize = 10.sp)
                Spacer(Modifier.height(2.dp))
                Text(flag.body, style = MaterialTheme.typography.bodySmall, color = c.onBg.copy(alpha = 0.8f))
            }
        }
        item("insights-bottom") { Spacer(Modifier.height(8.dp)) }
    }
}

// ─── Strength ───────────────────────────────────────────────────────────────

internal fun LazyListScope.strengthTab(state: StatsUiState, c: StatsColors) {
    state.e1rmLifts.firstOrNull()?.let { top ->
        item("e1rm-chart") { E1rmChartCard(top, c.onBg, c.muted, c.accent, c.outline) }
    }
    if (state.e1rmLifts.isNotEmpty()) {
        item("big-lifts") { E1rmCard(state.e1rmLifts, c.onBg, c.muted, c.accent, c.outline) }
    }
    state.repMaxes?.let { rm ->
        if (rm.entries.isNotEmpty()) item("repmax") { RepMaxCard(rm, c.onBg, c.muted, c.outline) }
    }
    if (state.compoundMaxes.isNotEmpty()) {
        cardItem("radar") { StrengthRadarCard(state.compoundMaxes) }
    }
    val withHistory = state.exerciseHistory.entries.filter { it.value.size >= 2 }.sortedByDescending { it.value.size }
    if (withHistory.size >= 2) {
        val a = withHistory[0]; val b = withHistory[1]
        cardItem("overlay") {
            StrengthOverlayCard(
                history1 = (Program.exercise(a.key)?.name ?: a.key) to a.value,
                history2 = (Program.exercise(b.key)?.name ?: b.key) to b.value
            )
        }
    }
    if (state.recentPrs.isNotEmpty()) {
        sectionTitle("pr-title", "Records broken", c)
        items(state.recentPrs, key = { "pr-${it.date}-${it.exerciseName}" }) { pr ->
            PrEntryRow(pr = pr, muted = c.muted, onBg = c.onBg, accent = c.accent,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 5.dp))
        }
    }
    sectionTitle("hof-title", "All-time bests", c)
    if (state.hallOfFame.isEmpty()) {
        item("hof-empty") {
            Text("No records yet. Finish a session to set your first PR.",
                style = MaterialTheme.typography.bodySmall, color = c.muted, fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp))
        }
    } else {
        items(state.hallOfFame.take(10), key = { "hof-${it.exerciseId}" }) { record ->
            HallOfFameRow(record = record, muted = c.muted, onBg = c.onBg, accent = c.accent,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 5.dp))
        }
    }
    if (state.timeToPr.isNotEmpty()) {
        cardItem("time-to-pr") { TimeToPrCard(state.timeToPr) }
    }
}

// ─── Volume ─────────────────────────────────────────────────────────────────

internal fun LazyListScope.volumeTab(state: StatsUiState, c: StatsColors) {
    if (state.volumeByMuscle.isNotEmpty()) {
        item("vol-bars") { StatsVolumeSection(rows = state.volumeByMuscle, muted = c.muted, accent = c.accent, outline = c.outline, onBg = c.onBg) }
        cardItem("vol-donut") { VolumeDonutCard(state.volumeByMuscle) }
        item("balance") { BalanceCheckCard(state.volumeByMuscle, c.onBg, c.muted, c.outline) }
    }
    if (state.weeklySetsByMuscle.isNotEmpty()) {
        item("vol-landmark") { VolumeLandmarkCard(state.weeklySetsByMuscle, c.onBg, c.muted, c.accent, c.outline) }
    }
    state.repRangeDist?.let { rr ->
        if (rr.total > 0) item("rep-range") { RepRangeCard(rr, c.onBg, c.muted, c.accent, c.outline) }
    }
    if (state.volumeDeloadTrend.isNotEmpty()) {
        cardItem("vol-deload") { VolumeDeloadCard(state.volumeDeloadTrend) }
    }
    if (state.dayTypeBestVsAvg.isNotEmpty()) {
        cardItem("daytype") { DayTypeBestVsAvgCard(state.dayTypeBestVsAvg) }
    }
    if (state.exerciseFrequency.isNotEmpty()) {
        item("freq") { StatsFreqSection(rows = state.exerciseFrequency, muted = c.muted, accent = c.accent, outline = c.outline, onBg = c.onBg) }
    }
    if (state.volumeByMuscle.isEmpty() && state.exerciseFrequency.isEmpty()) {
        item("vol-empty") {
            Text("No volume logged yet — finish a few sessions to see your muscle breakdown.",
                style = MaterialTheme.typography.bodySmall, color = c.muted, fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp, vertical = 16.dp))
        }
    }
}

// ─── Trends ─────────────────────────────────────────────────────────────────

internal fun LazyListScope.bodyTab(state: StatsUiState, c: StatsColors) {
    item("bodyweight") { BodyweightCard(state.bodyweightTrend, c.onBg, c.muted, c.accent, c.outline) }
    item("standards") { StrengthStandardsCard(state.e1rmLifts, state.bodyweightTrend.lastOrNull(), c.onBg, c.muted, c.accent, c.outline) }
}

internal fun LazyListScope.trendsTab(state: StatsUiState, c: StatsColors) {
    var any = false
    if (state.weeklySessionCounts.isNotEmpty()) { any = true; item("consistency") { ConsistencyHeatmapCard(state.weeklySessionCounts, 3, c.onBg, c.muted, c.accent, c.outline) } }
    if (state.avgRpePerSession.size >= 2) { any = true; item("rpe-trend") { RpeTrendCard(state.avgRpePerSession, state.avgRpe, c.onBg, c.muted, c.accent, c.outline) } }
    if (state.rpeDistribution.isNotEmpty()) { any = true; item("rpe-dist") { RpeCard(state.rpeDistribution, state.avgRpe, c.onBg, c.muted, c.accent, c.outline) } }
    if (state.prsByDayOfWeek.any { it > 0 }) { any = true; cardItem("pr-dow") { PrDayOfWeekCard(state.prsByDayOfWeek) } }
    if (state.prSessionTimestamps.size >= 3) { any = true; cardItem("pr-cluster") { PrClusteringCard(state.prSessionTimestamps) } }
    if (state.moodOverTime.size >= 3) { any = true; cardItem("mood") { EffortOverTimeCard(state.moodOverTime) } }
    if (!any) {
        item("trends-empty") {
            Text("Patterns show up here once you've trained for a few weeks.",
                style = MaterialTheme.typography.bodySmall, color = c.muted, fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp, vertical = 16.dp))
        }
    }
}
