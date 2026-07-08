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
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.gym.stats.components.BodyHeatmap
import com.forge.app.ui.gym.stats.state.PrRecord
import com.forge.app.ui.gym.stats.state.StatsUiState
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * The Stats hero + the always-on Records list. The hero mirrors the session-detail header — the
 * page's "face": THIS WEEK's three serif figures (volume · sessions · sets, with vs-last deltas) on
 * the left, the compact weekly muscle map on the right, and the one-line readiness status underneath
 * (2026-07-01 fusion — the old top-lift hero dissolved into the Strength lens, where it's simply the
 * first row).
 */
@Composable
internal fun ColumnScope.StatsHeroContent(state: StatsUiState, useKg: Boolean, c: StatsColors) {
    val cmp = state.weekComparison
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
        Column(Modifier.weight(1f)) {
            Text(
                "THIS WEEK",
                style = MaterialTheme.typography.labelMedium,
                color = c.muted,
                letterSpacing = 1.sp,
                modifier = Modifier.semantics { heading() }
            )
            Spacer(Modifier.height(12.dp))
            if (cmp != null) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    WeekMetric(
                        formatVolumeCompact(cmp.current.volumeLb, useKg, withUnit = false),
                        cmp.volumeDelta.takeIf { it != 0.0 }?.let { formatVolumeCompact(abs(it), useKg, withUnit = false) to (it > 0) },
                        unitLabel(useKg), c, Modifier.weight(1.1f)
                    )
                    WeekMetric("${cmp.current.sessions}", countDelta(cmp.sessionsDelta), "sessions", c, Modifier.weight(1f))
                    WeekMetric("${cmp.current.sets}", countDelta(cmp.current.sets - cmp.previous.sets), "sets", c, Modifier.weight(1f))
                }
            } else {
                // First run / a quiet week: honest zeros, drawn as the same three figures (§12) — never a
                // dash or a bare sentence. Deltas are suppressed (no prior week to compare against yet).
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    WeekMetric(formatVolumeCompact(0.0, useKg, withUnit = false), null, unitLabel(useKg), c, Modifier.weight(1.1f))
                    WeekMetric("0", null, "sessions", c, Modifier.weight(1f))
                    WeekMetric("0", null, "sets", c, Modifier.weight(1f))
                }
            }
        }
        // The weekly muscle map as the header's face — same spot as the session screen's body figure.
        // Always drawn: at zero it's the faint silhouette (the section's own visual at zero), so the hero
        // carries a mark before the first log instead of a gap (§12).
        Spacer(Modifier.width(14.dp))
        BodyHeatmap(
            setsByMuscle = state.weeklySetsByMuscle.associate { it.muscle to it.sets },
            accent = c.accent,
            faint = c.outline.copy(alpha = 0.34f),
            silhouette = c.outline.copy(alpha = 0.26f),
            labelColor = c.muted,
            figureHeight = 96.dp,
            showLegend = false,
            showTitles = false,
            modifier = Modifier.width(104.dp)
        )
    }
    if (state.readinessPulse != null && state.readinessThreshold != null) {
        Spacer(Modifier.height(14.dp))
        ReadinessLine(state.readinessPulse, state.readinessThreshold, c)
    }
}

/** The delta text + direction for a count metric; null = unchanged (no delta line shown). */
internal fun countDelta(d: Int): Pair<String, Boolean>? = if (d == 0) null else "${abs(d)}" to (d > 0)

/**
 * One week figure. The change vs last week sits on its OWN line under the label — a filled ▲/▼ plus
 * the actual amount ("▲ 2 vs last"), readable at a glance.
 */
@Composable
private fun WeekMetric(
    value: String,
    delta: Pair<String, Boolean>?,
    label: String,
    c: StatsColors,
    modifier: Modifier = Modifier
) {
    Column(modifier) {
        Text(
            value,
            style = MaterialTheme.typography.headlineMedium,
            color = c.onBg,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis
        )
        Spacer(Modifier.height(2.dp))
        Text(
            label.uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = c.muted,
            fontSize = 9.sp
        )
        delta?.let { (amount, up) ->
            Spacer(Modifier.height(4.dp))
            Text(
                "${if (up) "▲" else "▼"} $amount vs last",
                style = MaterialTheme.typography.labelSmall,
                color = if (up) c.accent else c.muted,
                fontSize = 9.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
        }
    }
}

/** Records — the all-time heaviest set per lift, biggest first. Rows tap through to the lift's trend. */
@Composable
internal fun ColumnScope.RecordsContent(
    records: List<PrRecord>,
    useKg: Boolean,
    c: StatsColors,
    onOpenLift: (String) -> Unit = {}
) {
    val fmt = remember { SimpleDateFormat("MMM d, yyyy", Locale.getDefault()) }
    val unit = unitLabel(useKg)
    val shown = records.take(6)
    shown.forEachIndexed { i, r ->
        Row(
            Modifier.fillMaxWidth()
                .clickableLabeled("Show estimated 1RM trend for ${r.exerciseName}") { onOpenLift(r.exerciseId) }
                .padding(vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(Modifier.weight(1f)) {
                Text(r.exerciseName, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
                Text(fmt.format(Date(r.sessionDate)), style = MaterialTheme.typography.labelSmall, color = c.muted)
            }
            Text(
                "${toDisplayWeight(r.maxWeightLb, useKg).roundToInt()} $unit × ${r.bestReps}",
                style = MaterialTheme.typography.titleSmall,
                color = c.accent
            )
        }
        // Table rule between record rows — a data line on the §5 hairline rung.
        if (i < shown.lastIndex) androidx.compose.material3.HorizontalDivider(color = c.outline.copy(alpha = 0.25f))
    }
}
