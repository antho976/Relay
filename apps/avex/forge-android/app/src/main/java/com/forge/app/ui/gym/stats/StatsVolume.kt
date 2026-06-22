package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.gym.stats.components.DivergingBar
import com.forge.app.ui.gym.stats.components.LineChart
import com.forge.app.ui.gym.stats.components.TargetBar
import com.forge.app.ui.gym.stats.components.statsEntrance
import com.forge.app.ui.gym.stats.state.BalanceRatioUi
import com.forge.app.ui.gym.stats.state.MuscleSetCount
import com.forge.app.ui.gym.stats.state.WeeklyTonnage
import kotlin.math.roundToInt

/** One muscle's "did I hit the target" row data — actual sets this week vs the planned target. */
private data class MuscleSetRow(val muscle: MuscleGroup, val actual: Int, val target: Int)

private fun muscleRows(
    weekly: List<MuscleSetCount>,
    planned: Map<MuscleGroup, Int>
): List<MuscleSetRow> {
    val actualBy = weekly.associate { it.muscle to it.sets }
    val muscles = (planned.keys + actualBy.keys).toSortedSet(compareBy { it.ordinal })
    return muscles.map { MuscleSetRow(it, actualBy[it] ?: 0, planned[it] ?: 0) }
}

/**
 * Tier 2a — sets per muscle this week against a per-muscle target tick. "Am I doing enough?" at a
 * glance: bars hitting their tick are on plan, short bars are the neglected areas.
 */
internal fun LazyListScope.setsPerMuscleSection(
    weekly: List<MuscleSetCount>,
    planned: Map<MuscleGroup, Int>,
    c: StatsColors
) {
    val rows = muscleRows(weekly, planned)
    if (rows.isEmpty()) return
    val maxV = rows.maxOf { maxOf(it.actual, it.target) }.coerceAtLeast(1)
    itemsIndexed(rows, key = { _, r -> "setsmuscle-${r.muscle.code}" }) { i, r ->
        MuscleSetBarRow(r, maxV, c, i)
    }
}

@Composable
private fun MuscleSetBarRow(r: MuscleSetRow, maxV: Int, c: StatsColors, index: Int) {
    val onPlan = r.target > 0 && r.actual >= r.target
    Row(
        Modifier
            .fillMaxWidth()
            .statsEntrance(index)
            .padding(horizontal = STATS_GUTTER, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            r.muscle.displayName,
            style = MaterialTheme.typography.bodySmall,
            color = c.onBg,
            modifier = Modifier.width(82.dp)
        )
        TargetBar(
            fraction = r.actual.toFloat() / maxV,
            targetFraction = if (r.target > 0) r.target.toFloat() / maxV else null,
            fillColor = if (onPlan) c.accent else c.accent.copy(alpha = 0.45f),
            trackColor = c.outline.copy(alpha = 0.18f),
            tickColor = c.onBg.copy(alpha = 0.55f),
            modifier = Modifier.weight(1f).height(14.dp)
        )
        Text(
            if (r.target > 0) "${r.actual}/${r.target}" else "${r.actual}",
            style = MaterialTheme.typography.labelSmall,
            color = if (onPlan) c.accent else c.muted,
            textAlign = TextAlign.End,
            modifier = Modifier.width(48.dp).padding(start = 8.dp)
        )
    }
}

/**
 * Tier 2b — structural balance as diverging bars (push/pull, quad/ham). Paired sides from a midline,
 * NOT a radar: asymmetry reads directly as imbalance and the two sides are honestly comparable.
 */
internal fun LazyListScope.balanceSection(ratios: List<BalanceRatioUi>, c: StatsColors) {
    // Key by index, not title — two ratios sharing a title would crash LazyColumn on a duplicate key.
    itemsIndexed(ratios, key = { i, _ -> "balance-$i" }) { i, b ->
        BalanceRow(b, c, i)
    }
}

@Composable
private fun BalanceRow(b: BalanceRatioUi, c: StatsColors, index: Int) {
    Column(
        Modifier
            .fillMaxWidth()
            .statsEntrance(index)
            .padding(horizontal = STATS_GUTTER, vertical = 8.dp)
    ) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(b.title, style = MaterialTheme.typography.labelMedium, color = c.onBg)
            val verdict = when (b.balanced) {
                true -> "balanced"
                false -> "skewed"
                null -> "—"
            }
            Text(verdict, style = MaterialTheme.typography.labelSmall,
                color = if (b.balanced == false) c.muted else c.accent)
        }
        Spacer(Modifier.height(6.dp))
        DivergingBar(
            leftValue = b.setsA,
            rightValue = b.setsB,
            leftColor = c.accent,
            rightColor = c.accent.copy(alpha = 0.45f),
            midlineColor = c.onBg.copy(alpha = 0.4f),
            modifier = Modifier.fillMaxWidth().height(16.dp)
        )
        Spacer(Modifier.height(4.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("${b.labelA} · ${b.setsA}", style = MaterialTheme.typography.labelSmall, color = c.muted)
            Text("${b.setsB} · ${b.labelB}", style = MaterialTheme.typography.labelSmall, color = c.muted)
        }
    }
}

/** Tonnage axis label: "12k lb" for ≥1000, but the exact value below that so a beginner's first weeks
 *  don't both read as a meaningless "0k". */
private fun tonnageLabel(v: Double, unit: String): String =
    if (v >= 1000) "${(v / 1000).roundToInt()}k $unit" else "${v.roundToInt()} $unit"

/**
 * Tier 2c — weekly tonnage trend, the "is it holding?" companion to the target bars. Deload weeks
 * dip by design, so the line is context, not a verdict.
 */
internal fun LazyListScope.tonnageTrendSection(
    weeklyTonnage: List<WeeklyTonnage>,
    useKg: Boolean,
    c: StatsColors
) {
    if (weeklyTonnage.size < 2) return
    item("tonnage-trend") {
        val display = weeklyTonnage.map { toDisplayWeight(it.volumeLb, useKg) }
        val lo = display.min()
        val hi = display.max()
        val unit = unitLabel(useKg)
        Column(
            Modifier.fillMaxWidth().statsEntrance(0).padding(horizontal = STATS_GUTTER, vertical = 8.dp)
        ) {
            LineChart(
                values = display,
                lineColor = c.accent,
                trendColor = c.muted,
                minValue = lo,
                maxValue = hi,
                modifier = Modifier.fillMaxWidth().height(120.dp)
            )
            Spacer(Modifier.height(6.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Text(tonnageLabel(lo, unit), style = MaterialTheme.typography.labelSmall, color = c.muted)
                Text("weekly volume · last ${weeklyTonnage.size} wks", style = MaterialTheme.typography.labelSmall, color = c.muted)
                Text(tonnageLabel(hi, unit), style = MaterialTheme.typography.labelSmall, color = c.muted)
            }
        }
    }
}
