package com.forge.app.ui.gym.stats

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.gym.stats.components.LineChart
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
 * Tier 2a — sets per muscle this week. Each muscle's TRACK LENGTH is its weekly target (relative to
 * the biggest target) and the fill is progress toward it — "fill the bar = hit the plan". Replaces
 * the old shared-scale bars with floating target ticks, which read as confusing stray lines.
 */
@Composable
internal fun ColumnScope.SetsPerMuscleContent(
    weekly: List<MuscleSetCount>,
    planned: Map<MuscleGroup, Int>,
    c: StatsColors
) {
    val rows = muscleRows(weekly, planned)
    if (rows.isEmpty()) return
    val maxTarget = rows.maxOf { maxOf(it.target, it.actual) }.coerceAtLeast(1)
    rows.forEach { r -> MuscleSetBarRow(r, maxTarget, c) }
}

@Composable
private fun MuscleSetBarRow(r: MuscleSetRow, maxTarget: Int, c: StatsColors) {
    val onPlan = r.target > 0 && r.actual >= r.target
    Row(
        Modifier.fillMaxWidth().padding(vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            r.muscle.displayName,
            style = MaterialTheme.typography.bodySmall,
            color = c.onBg,
            modifier = Modifier.width(82.dp)
        )
        // Track = this muscle's target; fill = progress. A muscle with no planned target sizes its
        // track by the work actually done instead (fully filled — there's no plan to fall short of).
        val trackFrac = when {
            r.target > 0 -> r.target.toFloat() / maxTarget
            else -> (r.actual.toFloat() / maxTarget).coerceAtMost(1f)
        }.coerceIn(0.12f, 1f)
        val fillFrac = when {
            r.target > 0 -> (r.actual.toFloat() / r.target).coerceIn(0f, 1f)
            r.actual > 0 -> 1f
            else -> 0f
        }
        Box(Modifier.weight(1f)) {
            Box(
                Modifier.fillMaxWidth(trackFrac).height(12.dp)
                    .clip(RoundedCornerShape(50))
                    .background(c.outline.copy(alpha = 0.18f))
            ) {
                if (fillFrac > 0f) Box(
                    Modifier.fillMaxWidth(fillFrac).fillMaxHeight()
                        .clip(RoundedCornerShape(50))
                        .background(if (onPlan) c.accent else c.accent.copy(alpha = 0.55f))
                )
            }
        }
        Text(
            if (r.target > 0) "${r.actual}/${r.target}" else "${r.actual}",
            style = MaterialTheme.typography.labelSmall,
            color = if (onPlan) c.accent else c.muted,
            textAlign = TextAlign.End,
            modifier = Modifier.width(48.dp).padding(start = 8.dp)
        )
    }
}

/** A pair needs at least this many combined sets before a ratio means anything — below it, 2-vs-13
 *  weeks scream "87% skewed" over noise and empty pairs render as broken "0 · 0" rows. */
internal const val MIN_BALANCE_SETS = 6

/**
 * Tier 2b — structural balance (push/pull, quad/ham) as one slim SPLIT bar per pair: the boundary
 * between the bright and dim halves IS the actual ratio, a center tick marks the 50/50 ideal, and
 * the verdict names the direction ("leans pull") instead of a bare "skewed". Pairs without enough
 * sets ([MIN_BALANCE_SETS]) don't render at all.
 */
@Composable
internal fun ColumnScope.BalanceContent(ratios: List<BalanceRatioUi>, c: StatsColors) {
    ratios.filter { it.setsA + it.setsB >= MIN_BALANCE_SETS }.forEach { b -> BalanceRow(b, c) }
}

@Composable
private fun BalanceRow(b: BalanceRatioUi, c: StatsColors) {
    val total = b.setsA + b.setsB
    Column(Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(b.title, style = MaterialTheme.typography.labelMedium, color = c.onBg)
            val verdict = when (b.balanced) {
                true -> "balanced"
                false -> "leans ${(if (b.setsA >= b.setsB) b.labelA else b.labelB).lowercase()}"
                null -> "—"
            }
            Text(verdict, style = MaterialTheme.typography.labelSmall,
                color = if (b.balanced == false) c.muted else c.accent)
        }
        Spacer(Modifier.height(8.dp))
        val fracA = b.setsA.toFloat() / total
        Box(
            Modifier.fillMaxWidth().height(8.dp)
                .clip(RoundedCornerShape(50))
                .background(c.outline.copy(alpha = 0.15f))
        ) {
            Row(Modifier.matchParentSize()) {
                if (fracA > 0f) Box(Modifier.weight(fracA).fillMaxHeight().background(c.accent))
                if (fracA < 1f) Box(Modifier.weight(1f - fracA).fillMaxHeight().background(c.accent.copy(alpha = 0.3f)))
            }
            // The 50/50 ideal — the further the color boundary sits from this tick, the more skewed.
            Box(
                Modifier.align(Alignment.Center).width(2.dp).fillMaxHeight()
                    .background(c.onBg.copy(alpha = 0.55f))
            )
        }
        Spacer(Modifier.height(6.dp))
        // One count per side — the bar already IS the ratio, so no duplicate percentages.
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("${b.labelA.uppercase()} · ${b.setsA} SETS", style = MaterialTheme.typography.labelSmall, color = c.muted, fontSize = 9.sp)
            Text("${b.setsB} SETS · ${b.labelB.uppercase()}", style = MaterialTheme.typography.labelSmall, color = c.muted, fontSize = 9.sp)
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
@Composable
internal fun ColumnScope.TonnageTrendContent(
    weeklyTonnage: List<WeeklyTonnage>,
    useKg: Boolean,
    c: StatsColors
) {
    if (weeklyTonnage.size < 2) return
    val display = weeklyTonnage.map { toDisplayWeight(it.volumeLb, useKg) }
    val lo = display.min()
    val hi = display.max()
    val unit = unitLabel(useKg)
    LineChart(
        values = display,
        lineColor = c.accent,
        trendColor = c.muted,
        minValue = lo,
        maxValue = hi,
        modifier = Modifier.fillMaxWidth().height(STATS_CHART_H)
    )
    Spacer(Modifier.height(6.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        Text(tonnageLabel(lo, unit), style = MaterialTheme.typography.labelSmall, color = c.muted)
        Text("weekly volume · last ${weeklyTonnage.size} wks", style = MaterialTheme.typography.labelSmall, color = c.muted)
        Text(tonnageLabel(hi, unit), style = MaterialTheme.typography.labelSmall, color = c.muted)
    }
}
