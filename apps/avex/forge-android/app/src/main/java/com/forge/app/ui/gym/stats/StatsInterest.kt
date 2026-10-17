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
import com.forge.app.ui.gym.stats.components.rememberToday
import com.forge.app.ui.gym.stats.state.DayLoad
import com.forge.app.ui.gym.stats.state.RpeBucket
import java.time.LocalDate
import kotlin.math.exp
import kotlin.math.abs
import kotlin.math.roundToInt
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.graphics.drawscope.Fill
import com.forge.app.ui.common.rirLabel
import com.forge.app.ui.common.rpeLabel
import java.time.format.DateTimeFormatter

/** Where an RPE sits on the effort axis. RPE 7 to 9 is the 1 to 3 reps-in-reserve band. */
private enum class EffortZone(val label: String, val left: String) {
    EASY("Easy", "4+ left"),
    PRODUCTIVE("In range", "1 to 3 left"),
    ALL_OUT("All out", "0 left");

    companion object {
        fun of(rpe: Double) = when {
            rpe < 7.0 -> EASY
            rpe <= 9.0 -> PRODUCTIVE
            else -> ALL_OUT
        }
    }
}

/**
 * Effort as an RPE histogram, made readable (Antho, 2026-10-17: "the Effort/RPE could be made more
 * clearer, but overall I like the chart").
 *
 * The bars stay. What they lacked was a scale anyone could read without already knowing RPE: each
 * bar now carries its set count, the RPE axis is translated into reps left in the tank underneath,
 * the 7 to 9 band most training is meant to land in is the only one drawn in full accent, and the
 * section opens on the one reading that matters, the share of rated sets inside that band.
 */
@Composable
internal fun ColumnScope.RpeHistogramContent(buckets: List<RpeBucket>, avgRpe: Double?, c: StatsColors) {
    if (buckets.isEmpty()) return
    val sorted = buckets.sortedBy { it.rpe }
    val max = sorted.maxOf { it.count }.coerceAtLeast(1)
    val total = sorted.sumOf { it.count }.coerceAtLeast(1)
    val inRange = sorted.filter { EffortZone.of(it.rpe) == EffortZone.PRODUCTIVE }.sumOf { it.count }
    val pct = (inRange * 100f / total).roundToInt()

    Row(verticalAlignment = Alignment.Bottom) {
        Text("$pct%", style = MaterialTheme.typography.headlineMedium, color = c.onBg)
        Spacer(Modifier.width(8.dp))
        Text(
            "of $total rated ${if (total == 1) "set" else "sets"} at RPE 7 to 9, the 1 to 3 reps-left range",
            style = MaterialTheme.typography.bodySmall,
            color = c.muted,
            modifier = Modifier.padding(bottom = 6.dp).weight(1f)
        )
    }
    Spacer(Modifier.height(14.dp))
    // The fixed height belongs to the bars, not the row: a row holding the bars AND their labels
    // clipped the labels at large font scales (§14). The labels take what they need.
    Row(
        Modifier
            .fillMaxWidth()
            .chartReadout(
                "Sets by RPE: " + sorted.joinToString("; ") { b ->
                    "RPE ${rpeLabel(b.rpe)}, ${rirLabel(b.rpe)} reps left, ${b.count} ${if (b.count == 1) "set" else "sets"}"
                } + ". $pct percent inside RPE 7 to 9."
            ),
        verticalAlignment = Alignment.Bottom,
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        sorted.forEach { b ->
            val zone = EffortZone.of(b.rpe)
            Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                Text("${b.count}", style = MaterialTheme.typography.labelSmall, color = if (zone == EffortZone.PRODUCTIVE) c.onBg else c.muted)
                Spacer(Modifier.height(3.dp))
                Box(Modifier.height(64.dp), contentAlignment = Alignment.BottomCenter) {
                    Box(
                        Modifier
                            .width(18.dp)
                            .height((b.count.toFloat() / max * 64f).dp.coerceAtLeast(2.dp))
                            .clip(RoundedCornerShape(topStart = 4.dp, topEnd = 4.dp))
                            .background(if (zone == EffortZone.PRODUCTIVE) c.accent else c.muted.copy(alpha = 0.35f))
                    )
                }
                Spacer(Modifier.height(4.dp))
                Text(rpeLabel(b.rpe), style = MaterialTheme.typography.labelMedium, color = c.onBg)
            }
        }
    }
    Spacer(Modifier.height(8.dp))
    // The zones as brackets under the columns they cover, so the axis reads in reps left.
    val runs = buildList<Pair<EffortZone, Int>> {
        sorted.forEach { b ->
            val z = EffortZone.of(b.rpe)
            if (isNotEmpty() && last().first == z) set(lastIndex, z to last().second + 1) else add(z to 1)
        }
    }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        runs.forEach { (zone, n) ->
            Column(Modifier.weight(n.toFloat())) {
                Box(
                    Modifier.fillMaxWidth().height(2.dp).clip(RoundedCornerShape(1.dp))
                        .background(if (zone == EffortZone.PRODUCTIVE) c.accent else c.muted.copy(alpha = 0.35f))
                )
                Spacer(Modifier.height(4.dp))
                Text(zone.label.uppercase(), style = MaterialTheme.typography.labelSmall, color = if (zone == EffortZone.PRODUCTIVE) c.onBg else c.muted)
                Text(zone.left, style = MaterialTheme.typography.bodySmall, color = c.muted)
            }
        }
    }
    avgRpe?.let {
        Spacer(Modifier.height(12.dp))
        // Through the app's RPE format, rounded to a tenth first: "8", not "8.0" (Format.kt).
        val avg = kotlin.math.round(it * 10) / 10.0
        Text(
            "Average RPE ${rpeLabel(avg)}, about ${rirLabel(avg)} reps left per set.",
            style = MaterialTheme.typography.bodySmall,
            color = c.muted
        )
    }
}

// "Sessions/PRs by day-of-week" patterns chart REMOVED 2026-07-01 — Antho: knowing how many
// sessions you do per day isn't useful. The state fields still flow (cheap aggregation) but no UI
// renders them.

/** Days of training load the fitness/fatigue model reads at most. */
private const val BANISTER_MAX_DAYS = 182

/** The model's read of today, in words: form as a share of fitness. */
private fun formWord(fitness: Float, form: Float): Pair<String, String> {
    if (fitness <= 0f) return "No load yet" to "Train and the two lines start to separate."
    val share = form / fitness * 100f
    val n = abs(share).roundToInt()
    return when {
        share > 10f -> "Fresh" to "Fitness sits $n% above fatigue."
        share >= -15f -> "Balanced" to "Fitness and fatigue sit within $n% of each other."
        share >= -35f -> "Building" to "Fatigue sits $n% above fitness, as it does through hard weeks."
        else -> "Deep fatigue" to "Fatigue sits $n% above fitness. An easier week lets it clear."
    }
}

/**
 * Tier 8c — the Banister fitness/fatigue model: a slow decay curve (fitness) and a fast one (fatigue)
 * over training load, with form as their difference.
 *
 * Made readable 2026-10-17. The three lines shared one scale, form drew in full white over the
 * other two, and the window was always 26 weeks, so a new account saw a flat line across most of
 * the chart. Now the window starts at the first training day, both averages are corrected for
 * their cold start, fitness and fatigue have
 * the chart to themselves, form gets its own band underneath (above the line fresh, below it
 * fatigued), and the section opens on the model's read of today in words.
 */
@Composable
internal fun ColumnScope.BanisterContent(dailyActivity: List<DayLoad>, c: StatsColors) {
    if (dailyActivity.size < 5) return
    // The EWMA sweep is recomputed only when the data changes, not on every recomposition.
    val todayDate = rememberToday()
    val series = remember(dailyActivity, todayDate) {
        val today = todayDate.toEpochDay()
        val firstDay = dailyActivity.minOf { it.epochDay }
        // Draw from the first training day (at most 26 weeks back): before it there is no load to
        // model, and a run of empty weeks read as a flat line across most of the chart.
        val start = today - BANISTER_MAX_DAYS + 1
        // The first week of a corrected average is one or two sessions' load and swings wildly, so
        // it is left out of the drawing whenever there are three weeks or more to show.
        val drawFrom = (if (today - firstDay >= 21) firstDay + 7 else firstDay).coerceIn(start, today)
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
        // Both averages start cold at the first training day, and the slow one takes months to
        // warm up: uncorrected, eight steady weeks read as fatigue far above fitness for everyone.
        // Dividing by the weight seen so far (1 - a^n) removes that start-up bias, so a steady load
        // reads as two lines that meet and only a real change in load pulls them apart.
        var fitWeight = 0.0
        var fatWeight = 0.0
        for (day in start..today) {
            val load = loadByDay[day] ?: 0.0
            if (day >= firstDay) {
                f = f * aFit + (1 - aFit) * load
                g = g * aFat + (1 - aFat) * load
                fitWeight = fitWeight * aFit + (1 - aFit)
                fatWeight = fatWeight * aFat + (1 - aFat)
            }
            val fit = if (fitWeight > 0) f / fitWeight else 0.0
            val fat = if (fatWeight > 0) g / fatWeight else 0.0
            if (day >= drawFrom) { fitness.add(fit.toFloat()); fatigue.add(fat.toFloat()); form.add((fit - fat).toFloat()) }
        }
        BanisterSeries(fitness, fatigue, form, java.time.LocalDate.ofEpochDay(drawFrom))
    }
    val fitNow = series.fitness.lastOrNull() ?: 0f
    val formNow = series.form.lastOrNull() ?: 0f
    val (word, line) = formWord(fitNow, formNow)

    Text(word, style = MaterialTheme.typography.headlineSmall, color = c.onBg)
    Spacer(Modifier.height(2.dp))
    Text(line, style = MaterialTheme.typography.bodySmall, color = c.muted)
    Spacer(Modifier.height(14.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
        LegendDot("Fitness", c.accent, c.muted)
        LegendDot("Fatigue", c.muted, c.muted)
    }
    Spacer(Modifier.height(8.dp))
    BanisterChart(
        series.fitness, series.fatigue, c.accent, c.muted, c.outline.copy(alpha = 0.25f),
        Modifier.fillMaxWidth().height(STATS_CHART_H).chartReadout(
            "Fitness and fatigue since ${series.from.format(DateTimeFormatter.ofPattern("MMMM d"))}. " +
                "Today reads $word. $line"
        )
    )
    Spacer(Modifier.height(10.dp))
    Text("FORM", style = MaterialTheme.typography.labelSmall, color = c.muted)
    Spacer(Modifier.height(4.dp))
    FormBand(series.form, c, Modifier.fillMaxWidth().height(36.dp).chartReadout("Form, fitness minus fatigue. Above the line is fresh, below it is fatigued."))
    Spacer(Modifier.height(4.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        Text(series.from.format(DateTimeFormatter.ofPattern("MMM d")).uppercase(), style = MaterialTheme.typography.labelSmall, color = c.muted)
        Text("TODAY", style = MaterialTheme.typography.labelSmall, color = c.muted)
    }
}

private data class BanisterSeries(
    val fitness: List<Float>,
    val fatigue: List<Float>,
    val form: List<Float>,
    val from: java.time.LocalDate
)

@Composable
private fun LegendDot(label: String, dot: Color, textColor: Color) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Box(Modifier.width(12.dp).height(3.dp).clip(RoundedCornerShape(2.dp)).background(dot))
        Text(label, style = MaterialTheme.typography.bodySmall, color = textColor)
    }
}

@Composable
private fun BanisterChart(
    fitness: List<Float>,
    fatigue: List<Float>,
    fitnessColor: Color,
    fatigueColor: Color,
    gridColor: Color,
    modifier: Modifier
) {
    val all = fitness + fatigue
    if (all.isEmpty()) return
    // Scaled to the data, not from zero: the story is the gap between the lines, and a zero floor
    // pressed both into the top third of the chart.
    val lo = all.min()
    val hi = all.max()
    val pad = ((hi - lo) * 0.08f).coerceAtLeast(1e-3f)
    val min = lo - pad
    val range = (hi + pad - min).coerceAtLeast(1e-3f)
    Canvas(modifier) {
        fun yOf(v: Float) = size.height - (v - min) / range * size.height
        drawLine(gridColor, Offset(0f, size.height), Offset(size.width, size.height), strokeWidth = 1.dp.toPx())
        fun pathOf(series: List<Float>): Path {
            val p = Path()
            val stepX = if (series.size > 1) size.width / (series.size - 1) else size.width
            series.forEachIndexed { i, v ->
                val x = stepX * i
                if (i == 0) p.moveTo(x, yOf(v)) else p.lineTo(x, yOf(v))
            }
            return p
        }
        drawPath(pathOf(fatigue), color = fatigueColor.copy(alpha = 0.65f), style = Stroke(width = 1.5.dp.toPx()))
        drawPath(pathOf(fitness), color = fitnessColor, style = Stroke(width = 2.5.dp.toPx()))
        fitness.lastOrNull()?.let { v -> drawCircle(fitnessColor, 3.5.dp.toPx(), Offset(size.width, yOf(v))) }
    }
}

/** Form as a band around its zero line: accent above (fresh), muted below (fatigued). */
@Composable
private fun FormBand(form: List<Float>, c: StatsColors, modifier: Modifier) {
    if (form.isEmpty()) return
    val reach = form.maxOf { abs(it) }.coerceAtLeast(1e-3f)
    Canvas(modifier) {
        val mid = size.height / 2f
        val stepX = if (form.size > 1) size.width / (form.size - 1) else size.width
        fun area(positive: Boolean): Path {
            val p = Path()
            p.moveTo(0f, mid)
            form.forEachIndexed { i, v ->
                val clamped = if (positive) v.coerceAtLeast(0f) else v.coerceAtMost(0f)
                p.lineTo(stepX * i, mid - clamped / reach * mid)
            }
            p.lineTo(stepX * (form.size - 1), mid)
            p.close()
            return p
        }
        drawPath(area(true), c.accent.copy(alpha = 0.6f), style = Fill)
        drawPath(area(false), c.muted.copy(alpha = 0.35f), style = Fill)
        drawLine(c.outline.copy(alpha = 0.35f), Offset(0f, mid), Offset(size.width, mid), strokeWidth = 1.dp.toPx())
    }
}
