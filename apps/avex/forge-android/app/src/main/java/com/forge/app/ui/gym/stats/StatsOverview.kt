package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.gym.stats.components.LineChart
import com.forge.app.ui.gym.stats.state.E1rmLift
import com.forge.app.ui.gym.stats.state.LifetimeMetrics
import com.forge.app.ui.gym.stats.state.PeriodComparison
import com.forge.app.ui.gym.stats.state.PrRecord
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt

/**
 * Overview — the Stats landing tab. Built so the very first thing on screen is a populated, legible
 * read (the old screen opened on a near-empty Strength list). One headline lift, the lifetime
 * at-a-glance tiles, this-week-vs-last, and the records — each in its own quiet [StatsCard].
 */

/** Hero: the strongest lift's estimated 1RM as the headline number, with its trend underneath. */
@Composable
internal fun ColumnScope.OverviewHeroContent(lift: E1rmLift, useKg: Boolean, c: StatsColors) {
    val display = lift.history.map { toDisplayWeight(it, useKg) }
    val current = toDisplayWeight(lift.currentE1rm, useKg).roundToInt()
    val unit = unitLabel(useKg)
    Text(
        "TOP LIFT",
        style = MaterialTheme.typography.labelMedium,
        color = c.muted
    )
    Spacer(Modifier.height(4.dp))
    Text(lift.exerciseName, style = MaterialTheme.typography.titleMedium, color = c.onBg)
    Spacer(Modifier.height(4.dp))
    Row(verticalAlignment = Alignment.Bottom) {
        Text(
            "$current",
            style = MaterialTheme.typography.headlineLarge,
            color = c.accent,
            fontWeight = FontWeight.SemiBold
        )
        Spacer(Modifier.width(6.dp))
        Text(
            "$unit e1RM",
            style = MaterialTheme.typography.titleSmall,
            color = c.muted,
            modifier = Modifier.padding(bottom = 4.dp)
        )
        Spacer(Modifier.weight(1f))
        lift.monthlyPct?.let { pct ->
            Text(
                "%+.1f%%/mo".format(pct),
                style = MaterialTheme.typography.labelLarge,
                color = if (pct >= 0) c.accent else c.muted,
                modifier = Modifier.padding(bottom = 4.dp)
            )
        }
    }
    if (display.size >= 2) {
        val lo = display.min()
        val hi = display.max()
        Spacer(Modifier.height(12.dp))
        LineChart(
            values = display,
            lineColor = c.accent,
            trendColor = c.muted,
            minValue = lo,
            maxValue = hi,
            modifier = Modifier.fillMaxWidth().height(STATS_HERO_CHART_H)
        )
    }
}

/** Lifetime at-a-glance — four compact tiles. */
@Composable
internal fun ColumnScope.LifetimeTilesContent(
    lifetime: LifetimeMetrics,
    recordCount: Int,
    useKg: Boolean,
    c: StatsColors
) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        StatTile("${lifetime.totalSessions}", "sessions", c, Modifier.weight(1f))
        StatTile(formatVolumeCompact(lifetime.lifetimeVolumeLb, useKg, withUnit = false), unitLabel(useKg) + " lifted", c, Modifier.weight(1f))
        StatTile("$recordCount", "records", c, Modifier.weight(1f))
        StatTile("${lifetime.avgSetCount.roundToInt()}", "avg sets", c, Modifier.weight(1f))
    }
}

@Composable
private fun StatTile(value: String, label: String, c: StatsColors, modifier: Modifier = Modifier) {
    Column(modifier, horizontalAlignment = Alignment.CenterHorizontally) {
        Text(
            value,
            style = MaterialTheme.typography.titleLarge,
            color = c.onBg,
            fontWeight = FontWeight.SemiBold
        )
        Spacer(Modifier.height(2.dp))
        Text(label, style = MaterialTheme.typography.labelSmall, color = c.muted, textAlign = TextAlign.Center)
    }
}

/** This ISO week vs last — four metrics, each with the up/down delta against the prior week. */
@Composable
internal fun ColumnScope.WeekComparisonContent(cmp: PeriodComparison, useKg: Boolean, c: StatsColors) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        WeekMetric("${cmp.current.sessions}", cmp.sessionsDelta.toDouble(), "sessions", c, Modifier.weight(1f))
        WeekMetric(
            formatVolumeCompact(cmp.current.volumeLb, useKg, withUnit = false),
            cmp.volumeDelta, "volume", c, Modifier.weight(1f)
        )
        WeekMetric("${cmp.current.sets}", (cmp.current.sets - cmp.previous.sets).toDouble(), "sets", c, Modifier.weight(1f))
        WeekMetric("${cmp.current.prs}", cmp.prsDelta.toDouble(), "PRs", c, Modifier.weight(1f))
    }
}

@Composable
private fun WeekMetric(value: String, delta: Double, label: String, c: StatsColors, modifier: Modifier = Modifier) {
    Column(modifier, horizontalAlignment = Alignment.CenterHorizontally) {
        Text(value, style = MaterialTheme.typography.titleLarge, color = c.onBg, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(2.dp))
        val (arrow, color) = when {
            delta > 0 -> "▲" to c.accent
            delta < 0 -> "▼" to c.muted
            else -> "•" to c.muted
        }
        Text(
            if (delta == 0.0) "same" else "$arrow vs last",
            style = MaterialTheme.typography.labelSmall,
            color = color,
            textAlign = TextAlign.Center
        )
        Text(label, style = MaterialTheme.typography.labelSmall, color = c.muted, textAlign = TextAlign.Center)
    }
}

/** Records — the all-time heaviest set per lift, biggest first. */
@Composable
internal fun ColumnScope.RecordsContent(records: List<PrRecord>, useKg: Boolean, c: StatsColors) {
    val fmt = remember { SimpleDateFormat("MMM d, yyyy", Locale.getDefault()) }
    val unit = unitLabel(useKg)
    val shown = records.take(6)
    shown.forEachIndexed { i, r ->
        Row(
            Modifier.fillMaxWidth().padding(vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(Modifier.weight(1f)) {
                Text(r.exerciseName, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
                Text(fmt.format(Date(r.sessionDate)), style = MaterialTheme.typography.labelSmall, color = c.muted)
            }
            Text(
                "${toDisplayWeight(r.maxWeightLb, useKg).roundToInt()} $unit × ${r.bestReps}",
                style = MaterialTheme.typography.titleSmall,
                color = c.accent,
                fontWeight = FontWeight.SemiBold
            )
        }
        if (i < shown.lastIndex) HorizontalDivider(color = c.outline.copy(alpha = 0.18f))
    }
}
