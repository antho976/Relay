package com.forge.app.ui.gym.stats

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import com.forge.app.ui.gym.stats.state.DayLoad
import com.forge.app.ui.gym.stats.state.RpeBucket
import com.forge.app.ui.gym.stats.state.TrainingTimes
import java.time.LocalDate
import kotlin.math.exp

private val DOW = listOf("M", "T", "W", "T", "F", "S", "S")

private fun rpeLabel(r: Double) = if (r % 1.0 == 0.0) r.toInt().toString() else "%.1f".format(r)

/** Tier 8a — effort distribution as an RPE histogram. Mostly redundant with readiness, so: for interest. */
@Composable
internal fun ColumnScope.RpeHistogramContent(buckets: List<RpeBucket>, avgRpe: Double?, c: StatsColors) {
    if (buckets.isEmpty()) return
    val sorted = buckets.sortedBy { it.rpe }
    val max = sorted.maxOf { it.count }.coerceAtLeast(1)
    Row(
        Modifier.fillMaxWidth().height(72.dp),
        verticalAlignment = Alignment.Bottom,
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        sorted.forEach { b ->
            Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                Box(
                    Modifier
                        .width(16.dp)
                        .height((b.count.toFloat() / max * 52f).dp.coerceAtLeast(2.dp))
                        .clip(RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp))
                        .background(c.accent.copy(alpha = 0.7f))
                )
                Spacer(Modifier.height(3.dp))
                Text(rpeLabel(b.rpe), style = MaterialTheme.typography.labelSmall, color = c.muted)
            }
        }
    }
    avgRpe?.let {
        Spacer(Modifier.height(6.dp))
        Text("Average RPE ${"%.1f".format(it)}", style = MaterialTheme.typography.labelSmall, color = c.muted)
    }
}

/** Tier 8b — time-of-day & PRs-by-weekday. The beautiful-but-useless trap: charted small, as one-liners. */
@Composable
internal fun ColumnScope.TrainingPatternsContent(
    trainingTimes: TrainingTimes?,
    prsByDayOfWeek: List<Int>,
    c: StatsColors
) {
    if (trainingTimes == null && prsByDayOfWeek.all { it == 0 }) return
    trainingTimes?.let { tt ->
        if (tt.sessionsByDayOfWeek.any { it > 0 }) {
            MiniDowBars("Sessions by day", tt.sessionsByDayOfWeek, c)
            Spacer(Modifier.height(8.dp))
        }
        tt.bestHourLabel?.let {
            Text("Most productive time: $it", style = MaterialTheme.typography.bodySmall, color = c.muted)
            Spacer(Modifier.height(8.dp))
        }
    }
    if (prsByDayOfWeek.any { it > 0 }) {
        MiniDowBars("PRs by day", prsByDayOfWeek, c)
    }
}

@Composable
private fun MiniDowBars(label: String, counts: List<Int>, c: StatsColors) {
    val max = counts.maxOrNull()?.coerceAtLeast(1) ?: 1
    Text(label, style = MaterialTheme.typography.labelSmall, color = c.muted)
    Spacer(Modifier.height(4.dp))
    Row(
        Modifier.fillMaxWidth().height(40.dp),
        verticalAlignment = Alignment.Bottom,
        horizontalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        counts.take(7).forEachIndexed { i, n ->
            Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height((n.toFloat() / max * 26f).dp.coerceAtLeast(2.dp))
                        .clip(RoundedCornerShape(2.dp))
                        .background(c.accent.copy(alpha = 0.5f))
                )
                Text(DOW.getOrElse(i) { "" }, style = MaterialTheme.typography.labelSmall, color = c.muted)
            }
        }
    }
}

/**
 * Tier 8c — the Banister fitness/fatigue model: a slow decay curve (fitness) and a fast one (fatigue)
 * over training load, with form as their difference. The prettiest chart here and a genuine showpiece,
 * but kept strictly viz-only — it earns its place as a visual, not a decision gate.
 */
@Composable
internal fun ColumnScope.BanisterContent(dailyActivity: List<DayLoad>, c: StatsColors) {
    if (dailyActivity.size < 5) return
    // The 182-day EWMA sweep is recomputed only when the data changes, not on every recomposition.
    val series = remember(dailyActivity) {
        val today = LocalDate.now().toEpochDay()
        val start = today - 182 + 1
        val loadByDay = dailyActivity.associate { it.epochDay to it.volumeLb }
        // Normalized EWMA so fitness (slow) and fatigue (fast) share a load scale; form = fitness −
        // fatigue dips after hard blocks and recovers with rest — the classic shape.
        val aFit = exp(-1.0 / 42.0)
        val aFat = exp(-1.0 / 7.0)
        var f = 0.0
        var g = 0.0
        val fitness = ArrayList<Float>()
        val fatigue = ArrayList<Float>()
        val form = ArrayList<Float>()
        for (day in start..today) {
            val load = loadByDay[day] ?: 0.0
            f = f * aFit + (1 - aFit) * load
            g = g * aFat + (1 - aFat) * load
            fitness.add(f.toFloat()); fatigue.add(g.toFloat()); form.add((f - g).toFloat())
        }
        Triple(fitness, fatigue, form)
    }
    BanisterChart(series.first, series.second, series.third, c.accent, c.muted, c.onBg, c.outline.copy(alpha = 0.12f),
        Modifier.fillMaxWidth().height(STATS_CHART_H))
    Spacer(Modifier.height(6.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
        LegendDot("Fitness", c.accent, c.muted)
        LegendDot("Fatigue", c.muted, c.muted)
        LegendDot("Form", c.onBg, c.muted)
    }
}

@Composable
private fun LegendDot(label: String, dot: Color, textColor: Color) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        Box(Modifier.width(10.dp).height(3.dp).clip(RoundedCornerShape(2.dp)).background(dot))
        Text(label, style = MaterialTheme.typography.labelSmall, color = textColor)
    }
}

@Composable
private fun BanisterChart(
    fitness: List<Float>,
    fatigue: List<Float>,
    form: List<Float>,
    fitnessColor: Color,
    fatigueColor: Color,
    formColor: Color,
    gridColor: Color,
    modifier: Modifier
) {
    val all = fitness + fatigue + form
    if (all.isEmpty()) return
    val min = all.min()
    val max = all.max()
    val range = (max - min).coerceAtLeast(1e-3f)
    Canvas(modifier) {
        drawLine(gridColor, Offset(0f, size.height / 2), Offset(size.width, size.height / 2), strokeWidth = 1.dp.toPx())
        fun pathOf(series: List<Float>): Path {
            val p = Path()
            val stepX = if (series.size > 1) size.width / (series.size - 1) else size.width
            series.forEachIndexed { i, v ->
                val x = stepX * i
                val y = size.height - (v - min) / range * size.height
                if (i == 0) p.moveTo(x, y) else p.lineTo(x, y)
            }
            return p
        }
        drawPath(pathOf(fitness), color = fitnessColor, style = Stroke(width = 2.dp.toPx()))
        drawPath(pathOf(fatigue), color = fatigueColor.copy(alpha = 0.7f), style = Stroke(width = 1.5.dp.toPx()))
        drawPath(pathOf(form), color = formColor, style = Stroke(width = 1.5.dp.toPx()))
    }
}
