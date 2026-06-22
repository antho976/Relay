package com.forge.app.ui.gym.stats

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.forge.app.domain.adapt.E1rm
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.gym.stats.components.ScatterChart
import com.forge.app.ui.gym.stats.components.statsEntrance
import com.forge.app.ui.gym.stats.state.PrEntry
import com.forge.app.ui.gym.stats.state.StrengthCurve
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt

/**
 * Tier 6a — PR timeline: every personal record as an event marker along a time axis, with the most
 * recent few spelled out below. "When did the breakthroughs happen."
 */
internal fun LazyListScope.prTimelineSection(prs: List<PrEntry>, useKg: Boolean, c: StatsColors) {
    if (prs.isEmpty()) return
    item("pr-timeline") {
        val unit = unitLabel(useKg)
        // Sort once per data change, not every recomposition; one formatter reused for both rows.
        val byDate = remember(prs) { prs.sortedBy { it.date } }
        val timestamps = remember(byDate) { byDate.map { it.date } }
        val recent = remember(prs) { prs.sortedByDescending { it.date }.take(5) }
        val fmt = remember { SimpleDateFormat("MMM d", Locale.getDefault()) }
        Column(Modifier.fillMaxWidth().statsEntrance(0).padding(horizontal = STATS_GUTTER)) {
            if (byDate.size >= 2) {
                PrTimelineStrip(
                    timestamps = timestamps,
                    axisColor = c.outline.copy(alpha = 0.4f),
                    dotColor = c.accent,
                    modifier = Modifier.fillMaxWidth().height(36.dp)
                )
                Spacer(Modifier.height(4.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(fmt.format(Date(byDate.first().date)).uppercase(),
                        style = MaterialTheme.typography.labelSmall, color = c.muted)
                    Text("${byDate.size} PRs", style = MaterialTheme.typography.labelSmall, color = c.muted)
                    Text(fmt.format(Date(byDate.last().date)).uppercase(),
                        style = MaterialTheme.typography.labelSmall, color = c.muted)
                }
                Spacer(Modifier.height(12.dp))
            }
            recent.forEach { pr ->
                Row(
                    Modifier.fillMaxWidth().padding(vertical = 4.dp),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(pr.exerciseName, style = MaterialTheme.typography.bodySmall, color = c.onBg,
                        modifier = Modifier.weight(1f))
                    Text(
                        "${toDisplayWeight(pr.weightLb, useKg).roundToInt()} $unit × ${pr.reps} · ${fmt.format(Date(pr.date))}",
                        style = MaterialTheme.typography.labelSmall, color = c.muted
                    )
                }
            }
        }
    }
}

@Composable
private fun PrTimelineStrip(timestamps: List<Long>, axisColor: Color, dotColor: Color, modifier: Modifier) {
    val min = timestamps.min()
    val max = timestamps.max()
    val span = (max - min).coerceAtLeast(1L)
    Canvas(modifier) {
        val y = size.height / 2f
        drawLine(axisColor, Offset(0f, y), Offset(size.width, y), strokeWidth = 1.5.dp.toPx())
        timestamps.forEach { ts ->
            val x = ((ts - min).toFloat() / span) * size.width
            drawCircle(dotColor, radius = 3.5.dp.toPx(), center = Offset(x, y))
        }
    }
}

/**
 * Tier 6b — load-rep strength curve: every working set as a dot (weight vs reps), the Epley curve
 * fitted through them, and the extrapolated 1RM marked at one rep. The chart that shows the shape of
 * your strength — the visual payoff of the e1RM model.
 */
internal fun LazyListScope.strengthCurveSection(curves: List<StrengthCurve>, useKg: Boolean, c: StatsColors) {
    itemsIndexed(curves, key = { _, s -> "curve-${s.exerciseId}" }) { i, curve ->
        StrengthCurveRow(curve, useKg, c, i)
    }
}

@Composable
private fun StrengthCurveRow(curve: StrengthCurve, useKg: Boolean, c: StatsColors, index: Int) {
    val unit = unitLabel(useKg)
    val pts = curve.points.map { Offset(it.reps.toFloat(), toDisplayWeight(it.weightLb, useKg).toFloat()) }
    val e1 = toDisplayWeight(curve.e1rmLb, useKg).toFloat()
    val maxReps = curve.points.maxOf { it.reps }.coerceAtLeast(2)
    // Fitted curve from the e1RM via the shared Epley inverse — never diverges from E1rm.epley.
    val overlay = (maxReps downTo 1).map { r ->
        Offset(r.toFloat(), E1rm.epleyInverse(e1.toDouble(), r).toFloat())
    }
    val minY = minOf(pts.minOf { it.y }, overlay.minOf { it.y })
    val maxY = maxOf(e1, pts.maxOf { it.y })

    Column(Modifier.fillMaxWidth().statsEntrance(index).padding(horizontal = STATS_GUTTER, vertical = 8.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(curve.exerciseName, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
            Text("1RM ≈ ${e1.roundToInt()} $unit", style = MaterialTheme.typography.labelMedium, color = c.accent)
        }
        Spacer(Modifier.height(6.dp))
        ScatterChart(
            points = pts,
            overlay = overlay,
            pointColor = c.muted,
            lineColor = c.accent,
            gridColor = c.outline.copy(alpha = 0.12f),
            minX = 1f, maxX = maxReps.toFloat(),
            minY = minY * 0.95f, maxY = maxY * 1.05f,
            highlightOverlayEnd = true,
            modifier = Modifier.fillMaxWidth().height(150.dp)
        )
        Spacer(Modifier.height(4.dp))
        Text("every set · weight × reps — curve fitted, ● = projected 1-rep max",
            style = MaterialTheme.typography.labelSmall, color = c.muted)
    }
}
