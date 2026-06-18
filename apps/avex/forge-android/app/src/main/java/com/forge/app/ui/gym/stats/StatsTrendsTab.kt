package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.stats.components.EffortOverTimeCard
import com.forge.app.ui.gym.stats.components.EntranceItem
import com.forge.app.ui.gym.stats.components.PrDayOfWeekCard
import com.forge.app.ui.gym.stats.state.StatsUiState

// Verbatim titles from InsightEngine — these two observations render on Trends, under the
// cards that visualize the same signal, instead of in the Snapshot insights list.
internal const val INSIGHT_ESTIMATE_TITLE = "Estimate vs reality"
internal const val INSIGHT_MOOD_TITLE = "Mood moves your volume"

/**
 * The Trends tab: consistency grid → when-you-train → effort distribution → duration
 * trend (with the estimate-calibration insight as caption) → RPE → PRs by weekday →
 * mood line (with the mood×volume insight beneath).
 */
internal fun LazyListScope.trendsTab(state: StatsUiState, c: StatsColors) {
    var any = false
    var idx = 0
    if (state.weeklySessionCounts.any { it > 0 }) {
        any = true
        val i = idx++
        item("consistency") { EntranceItem(i) { ConsistencyHeatmapCard(state.weeklySessionCounts, 3, c.onBg, c.muted, c.accent, c.outline) } }
    }
    state.trainingTimes?.let { times ->
        if (times.sessionsByDayOfWeek.any { it > 0 }) {
            any = true
            val i = idx++
            item("when-you-train") { EntranceItem(i) { WhenYouTrainCard(times, c.onBg, c.muted, c.accent, c.outline) } }
        }
    }
    if (state.effortDistribution.count { it.total > 0 } >= 2) {
        any = true
        val i = idx++
        item("effort-dist") { EntranceItem(i) { EffortDistributionCard(state.effortDistribution, c.onBg, c.muted, c.accent, c.outline) } }
    }
    if (state.weeklyDurations.size >= 2) {
        any = true
        val i = idx++
        val calibration = state.insights.firstOrNull { it.title == INSIGHT_ESTIMATE_TITLE }
        item("duration") { EntranceItem(i) { DurationTrendCard(state.weeklyDurations, calibration, c.onBg, c.muted, c.accent, c.outline) } }
    }
    if (state.avgRpePerSession.size >= 2) {
        any = true
        val i = idx++
        item("rpe-trend") { EntranceItem(i) { RpeTrendCard(state.avgRpePerSession, state.avgRpe, c.onBg, c.muted, c.accent, c.outline) } }
    }
    if (state.rpeDistribution.isNotEmpty()) {
        any = true
        val i = idx++
        item("rpe-dist") { EntranceItem(i) { RpeCard(state.rpeDistribution, state.avgRpe, c.onBg, c.muted, c.accent, c.outline) } }
    }
    if (state.prsByDayOfWeek.any { it > 0 }) { any = true; cardItem("pr-dow") { PrDayOfWeekCard(state.prsByDayOfWeek) } }
    if (state.moodOverTime.size >= 3) {
        any = true
        cardItem("mood") { EffortOverTimeCard(state.moodOverTime) }
        state.insights.firstOrNull { it.title == INSIGHT_MOOD_TITLE }?.let { flag ->
            item("mood-link") {
                Text(
                    "${flag.emoji} ${flag.body}",
                    style = MaterialTheme.typography.bodySmall, color = c.accent, fontStyle = FontStyle.Italic,
                    modifier = Modifier.padding(horizontal = 24.dp, vertical = 6.dp)
                )
            }
        }
    }
    // The charts gated on a multi-week / multi-entry threshold — surfaced so a partial Trends tab
    // says what's still coming and what unlocks it, rather than just hiding the locked cards.
    val locked = buildList {
        if (state.effortDistribution.count { it.total > 0 } < 2) add("Effort distribution — rate effort across 2 weeks")
        if (state.weeklyDurations.size < 2) add("Duration trend — train across 2 weeks")
        if (state.avgRpePerSession.size < 2) add("RPE trend — log RPE on 2 sessions")
        if (state.moodOverTime.size < 3) add("Mood line — 3 mood check-ins")
    }
    if (!any) {
        item("trends-empty") {
            Column(Modifier.padding(horizontal = 24.dp, vertical = 16.dp)) {
                Text("No patterns yet — your Trends unlock as you train:",
                    style = MaterialTheme.typography.bodySmall, color = c.muted, fontStyle = FontStyle.Italic)
                Spacer(Modifier.height(8.dp))
                locked.forEach { Text("·  $it", style = MaterialTheme.typography.bodySmall, color = c.muted, fontSize = 11.sp) }
            }
        }
    } else if (locked.isNotEmpty()) {
        item("trends-locked") {
            Column(Modifier.padding(horizontal = 24.dp, vertical = 14.dp)) {
                Text("STILL TO UNLOCK", style = MaterialTheme.typography.labelSmall, color = c.muted, fontSize = 9.sp)
                Spacer(Modifier.height(6.dp))
                locked.forEach { Text("·  $it", style = MaterialTheme.typography.bodySmall, color = c.muted.copy(alpha = 0.8f), fontSize = 11.sp) }
            }
        }
    }
}
