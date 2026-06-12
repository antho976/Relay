package com.forge.app.ui.gym.stats

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.stats.components.DayTypeBestVsAvgCard
import com.forge.app.ui.gym.stats.components.EntranceItem
import com.forge.app.ui.gym.stats.components.RhythmRow
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
    val bg = MaterialTheme.colorScheme.background
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline

    // Each tab carries its OWN background pill (selected = filled, unselected = outline). The pill is
    // part of the chip's layout, so it always wraps the label exactly — unlike the old separately
    // drawn "sliding" pill, which was positioned in the Row's outer coordinate space and ended up
    // offset by the Row padding (the white bubble didn't line up with its text).
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        StatsTab.entries.forEach { tab ->
            val isSel = tab == selected
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(50))
                    .then(
                        if (isSel) Modifier.background(onBg)
                        else Modifier.border(1.dp, outline.copy(alpha = 0.5f), RoundedCornerShape(50))
                    )
                    .clickable { onSelect(tab) }
                    .padding(horizontal = 14.dp, vertical = 7.dp)
            ) {
                Text(
                    tab.label,
                    style = MaterialTheme.typography.labelMedium,
                    color = if (isSel) bg else muted,
                    fontWeight = if (isSel) FontWeight.SemiBold else FontWeight.Normal
                )
            }
        }
    }
    HorizontalDivider(color = outline.copy(alpha = 0.2f))
}

internal fun LazyListScope.sectionTitle(key: String, title: String, c: StatsColors) {
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
internal fun LazyListScope.cardItem(key: String, content: @Composable () -> Unit) {
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
        EntranceItem(0) {
            StatsHeroSection(
                weekNum = weekNum, weekLabel = weekLabel, weekSessions = weekSessions,
                weekCurrentVolumeLb = weekCurrent?.volumeLb, weekCurrentPrs = weekCurrent?.prs ?: 0,
                cardioMin = state.thisWeekCardioMin, onBg = c.onBg, muted = c.muted
            )
        }
    }
    item("momentum") { EntranceItem(1) { MomentumGrid(weekCurrent, weekPrev, c.onBg, c.muted, c.outline) } }
    item("highlights") { EntranceItem(2) { HighlightCards(state.consistencyStreakWeeks, state.progressiveOverloadPct, c.onBg, c.muted, c.outline) } }
    item("overload") { EntranceItem(3) { OverloadCard(state.overload, state.progressiveOverloadPct, c.onBg, c.muted, c.accent, c.outline) } }
    item("pulse") { EntranceItem(4) { PulseCard(state.readinessPulse, c.onBg, c.muted, c.outline) } }
    item("week-plan") {
        EntranceItem(5) {
            WeekVsPlanCard(
                actualSets = state.weeklySetsByMuscle.sumOf { it.sets },
                plannedSets = state.plannedSetsByMuscle.values.sum(),
                onBg = c.onBg, muted = c.muted, outline = c.outline
            )
        }
    }
    item("rhythm") {
        EntranceItem(6) {
            Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
                Text("RHYTHM", style = MaterialTheme.typography.labelSmall, color = c.muted, fontSize = 9.sp)
                Spacer(Modifier.height(8.dp))
                RhythmRow(weekActivity = state.weekActivity, today = today, onBg = c.onBg, muted = c.muted, outline = c.outline)
                Spacer(Modifier.height(20.dp))
                HorizontalDivider(color = c.outline.copy(alpha = 0.25f))
                Spacer(Modifier.height(20.dp))
            }
        }
    }
    sectionTitle("week-title", "What I did this week", c)
    itemsIndexed(state.weekActivity.ifEmpty { emptyWeekActivity() }, key = { _, row -> "day-${row.dayOfWeek}" }) { rowIdx, row ->
        EntranceItem(rowIdx.coerceAtMost(8)) {
            WeekDayRow(row = row, today = today, onBg = c.onBg, muted = c.muted, accent = c.accent,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 5.dp))
        }
    }
    item("lifetime") { EntranceItem(0) { StatsLifetimeSection(lm = state.lifetimeMetrics, onBg = c.onBg, muted = c.muted, outline = c.outline) } }
    // The estimate-calibration and mood×volume observations render on Trends instead,
    // under the cards that visualize those signals.
    val snapshotInsights = state.insights.filterNot { it.title == INSIGHT_ESTIMATE_TITLE || it.title == INSIGHT_MOOD_TITLE }
    if (snapshotInsights.isNotEmpty()) {
        sectionTitle("insights-title", "Insights", c)
        itemsIndexed(snapshotInsights, key = { _, flag -> "insight-${flag.title}" }) { rowIdx, flag ->
            EntranceItem(rowIdx.coerceAtMost(8)) {
                Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 6.dp)) {
                    Text(flag.title, style = MaterialTheme.typography.labelSmall, color = c.muted, fontSize = 10.sp)
                    Spacer(Modifier.height(2.dp))
                    Text(flag.body, style = MaterialTheme.typography.bodySmall, color = c.onBg.copy(alpha = 0.8f))
                }
            }
        }
        item("insights-bottom") { Spacer(Modifier.height(8.dp)) }
    }
}

// ─── Strength: see StatsStrengthTab.kt · Trends: see StatsTrendsTab.kt ─────────

// ─── Volume ─────────────────────────────────────────────────────────────────

internal fun LazyListScope.volumeTab(state: StatsUiState, c: StatsColors) {
    var idx = 0
    val volumeLbByMuscle = state.volumeByMuscle.associate { it.muscle to it.volumeLb }
    val hasTargets = state.plannedSetsByMuscle.isNotEmpty() || state.weeklySetsByMuscle.isNotEmpty()
    if (hasTargets) {
        // Renders the full plan even at zero data — the cold start shows targets, not a gap.
        val i = idx++
        item("muscle-target") {
            EntranceItem(i) {
                MuscleTargetSection(
                    actual = state.weeklySetsByMuscle, planned = state.plannedSetsByMuscle,
                    volumeLbByMuscle = volumeLbByMuscle,
                    onBg = c.onBg, muted = c.muted, accent = c.accent, outline = c.outline
                )
            }
        }
    }
    if (state.balanceRatios.isNotEmpty()) {
        val i = idx++
        item("balance-ratios") { EntranceItem(i) { BalanceRatiosSection(state.balanceRatios, c.onBg, c.muted, c.accent, c.outline) } }
    }
    if (state.weeklyTonnage.size >= 2) {
        val i = idx++
        item("tonnage") { EntranceItem(i) { TonnageTrendCard(state.weeklyTonnage, c.onBg, c.muted, c.accent, c.outline) } }
    }
    state.repRangeDist?.let { rr ->
        if (rr.total > 0) {
            val i = idx++
            item("rep-range") { EntranceItem(i) { RepRangeCard(rr, c.onBg, c.muted, c.accent, c.outline) } }
        }
    }
    if (state.dayTypeBestVsAvg.isNotEmpty()) {
        cardItem("daytype") { DayTypeBestVsAvgCard(state.dayTypeBestVsAvg) }
    }
    if (state.exerciseFrequency.isNotEmpty()) {
        val i = idx++
        item("freq") { EntranceItem(i) { StatsFreqSection(rows = state.exerciseFrequency, muted = c.muted, accent = c.accent, outline = c.outline, onBg = c.onBg) } }
    }
    if (!hasTargets && state.exerciseFrequency.isEmpty()) {
        item("vol-empty") {
            Text("No plan and no volume yet. Generate a program or log a session.",
                style = MaterialTheme.typography.bodySmall, color = c.muted, fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp, vertical = 16.dp))
        }
    }
}

// ─── Body ───────────────────────────────────────────────────────────────────

internal fun LazyListScope.bodyTab(state: StatsUiState, c: StatsColors) {
    item("bodyweight") { EntranceItem(0) { BodyweightCard(state.bodyweightPoints, c.onBg, c.muted, c.accent, c.outline) } }
    item("standards") {
        EntranceItem(1) {
            StrengthStandardsCard(
                state.e1rmLifts, state.bodyweightPoints.lastOrNull()?.weightLb, state.userSex,
                c.onBg, c.muted, c.accent, c.outline
            )
        }
    }
}

