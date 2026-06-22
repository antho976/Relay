package com.forge.app.ui.gym.stats

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
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.gym.stats.components.BandedBar
import com.forge.app.ui.gym.stats.components.ScatterChart
import com.forge.app.ui.gym.stats.components.statsEntrance
import com.forge.app.ui.gym.stats.state.BodyweightPoint
import com.forge.app.ui.gym.stats.state.E1rmLift
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt

// Bodyweight-relative strength tiers — recovered verbatim from the old StrengthStandardsCard so the
// "where you stand" read can't drift. Generic ×bodyweight bands (a simplification: real standards
// differ per lift, but honest for DB/machine work with no published tables). Sex-aware.
private val TIER_CUTOFFS_MALE = listOf(0.4, 0.7, 1.1, 1.5)
private val TIER_CUTOFFS_FEMALE = listOf(0.3, 0.5, 0.8, 1.1)
private val TIER_FULL = listOf("Untrained", "Novice", "Intermediate", "Advanced", "Elite")
private const val MIN_SESSIONS_FOR_TIER = 3

private fun tierCutoffs(sex: String) = if (sex == "female") TIER_CUTOFFS_FEMALE else TIER_CUTOFFS_MALE
private fun tierIndex(ratio: Double, sex: String): Int {
    tierCutoffs(sex).forEachIndexed { i, cut -> if (ratio < cut) return i }
    return tierCutoffs(sex).size
}

/**
 * Tier 5a — bodyweight as a moving-average line over the raw daily scatter. Daily weigh-ins are noisy,
 * so the smoothed line is the honest read; the dots keep the raw truth visible.
 */
internal fun LazyListScope.bodyweightSection(points: List<BodyweightPoint>, useKg: Boolean, c: StatsColors) {
    if (points.isEmpty()) return
    item("bodyweight") {
        val unit = unitLabel(useKg)
        // Display conversion + the O(n×window) moving-average sweep run once per data change, not on
        // every recomposition.
        val display = remember(points, useKg) { points.map { toDisplayWeight(it.weightLb, useKg) } }
        Column(Modifier.fillMaxWidth().statsEntrance(0).padding(horizontal = STATS_GUTTER)) {
            if (points.size < 2) {
                Text(
                    "Latest: ${display.last().roundToInt()} $unit. A few more weigh-ins and the trend line shows up.",
                    style = MaterialTheme.typography.bodySmall, color = c.muted, fontStyle = FontStyle.Italic
                )
            } else {
                val chart = remember(points, useKg) {
                    val day0 = points.first().recordedAt
                    val xs = points.map { ((it.recordedAt - day0) / 86_400_000.0).toFloat() }
                    val scatter = display.indices.map { Offset(xs[it], display[it].toFloat()) }
                    val window = minOf(7, points.size)
                    val ma = display.indices.map { i ->
                        val from = maxOf(0, i - window + 1)
                        Offset(xs[i], display.subList(from, i + 1).average().toFloat())
                    }
                    val lo = display.min().toFloat()
                    val hi = display.max().toFloat()
                    val pad = ((hi - lo) * 0.12f).coerceAtLeast(1f)
                    BwChartData(scatter, ma, xs.first(), xs.last(), lo - pad, hi + pad, window)
                }
                ScatterChart(
                    points = chart.scatter,
                    overlay = chart.ma,
                    pointColor = c.muted,
                    lineColor = c.accent,
                    gridColor = c.outline.copy(alpha = 0.12f),
                    minX = chart.minX, maxX = chart.maxX,
                    minY = chart.minY, maxY = chart.maxY,
                    modifier = Modifier.fillMaxWidth().height(150.dp)
                )
                Spacer(Modifier.height(6.dp))
                val fmt = remember { SimpleDateFormat("MMM d", Locale.getDefault()) }
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(fmt.format(Date(points.first().recordedAt)).uppercase(),
                        style = MaterialTheme.typography.labelSmall, color = c.muted)
                    Text("now ${display.last().roundToInt()} $unit · ${chart.window}-pt avg line",
                        style = MaterialTheme.typography.labelSmall, color = c.muted)
                    Text(fmt.format(Date(points.last().recordedAt)).uppercase(),
                        style = MaterialTheme.typography.labelSmall, color = c.muted)
                }
            }
        }
    }
}

/** Precomputed bodyweight scatter + moving-average overlay + axis bounds for one render (see above). */
private class BwChartData(
    val scatter: List<Offset>,
    val ma: List<Offset>,
    val minX: Float,
    val maxX: Float,
    val minY: Float,
    val maxY: Float,
    val window: Int
)

/**
 * Tier 5b — relative strength: each main lift's e1RM ÷ bodyweight as a marker sitting on banded tier
 * zones (Untrained → Elite). "Where do I rank" at a glance. A tier only locks once a lift has a few
 * sessions behind its e1RM, so one fluke set can't read as Advanced.
 */
internal fun LazyListScope.strengthStandardsSection(
    lifts: List<E1rmLift>,
    bodyweightLb: Double?,
    sex: String,
    c: StatsColors
) {
    val bw = bodyweightLb ?: 0.0
    if (bw <= 0.0) {
        item("standards-nobw") {
            Text(
                "Log your bodyweight to see where you stand.",
                style = MaterialTheme.typography.bodySmall, color = c.muted, fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = STATS_GUTTER, vertical = 8.dp)
            )
        }
        return
    }
    val rated = lifts.filter { it.currentE1rm > 0 && it.history.size >= MIN_SESSIONS_FOR_TIER }.take(5)
    if (rated.isEmpty()) {
        item("standards-calibrating") {
            Text(
                "Still calibrating — a few more sessions of your main lifts and your tier locks in.",
                style = MaterialTheme.typography.bodySmall, color = c.muted, fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = STATS_GUTTER, vertical = 8.dp)
            )
        }
        return
    }
    itemsIndexed(rated, key = { _, l -> "std-${l.exerciseId}" }) { i, lift ->
        StrengthStandardRow(lift, bw, sex, c, i)
    }
}

@Composable
private fun StrengthStandardRow(lift: E1rmLift, bw: Double, sex: String, c: StatsColors, index: Int) {
    val ratio = lift.currentE1rm / bw
    val idx = tierIndex(ratio, sex)
    val cutoffs = tierCutoffs(sex)
    val maxRatio = (cutoffs.last() * 1.3).toFloat()
    val zoneEdges = cutoffs.map { (it / maxRatio).toFloat() }
    val zoneColors = (0..4).map { lerp(c.outline.copy(alpha = 0.30f), c.accent, it / 4f) }

    Column(
        Modifier.fillMaxWidth().statsEntrance(index).padding(horizontal = STATS_GUTTER, vertical = 8.dp)
    ) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(lift.exerciseName, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
            Text(
                "%.2f× BW · %s".format(ratio, TIER_FULL[idx]),
                style = MaterialTheme.typography.labelMedium, color = c.onBg, fontWeight = FontWeight.SemiBold
            )
        }
        Spacer(Modifier.height(6.dp))
        BandedBar(
            markerFraction = (ratio / maxRatio).toFloat(),
            zoneEdges = zoneEdges,
            zoneColors = zoneColors,
            markerColor = c.onBg,
            modifier = Modifier.fillMaxWidth().height(12.dp)
        )
    }
}
