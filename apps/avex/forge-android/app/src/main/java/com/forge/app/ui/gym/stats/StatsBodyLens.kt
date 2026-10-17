package com.forge.app.ui.gym.stats

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.ui.common.ConnectWatchCard
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.gym.stats.state.StatsUiState
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * The Body lens (2026-10-16): what the watch says about recovery, next to the training it recovers
 * from, plus bodyweight. Stats used to show no watch data at all, so the only way to see a night's
 * sleep or a resting-HR trend was the Coach tab's evidence list.
 */
internal fun LazyListScope.bodyLens(
    state: StatsUiState,
    body: StatsBodyUi?,
    weightUnit: WeightUnit,
    c: StatsColors,
    onConnectWatch: () -> Unit,
    onOpenProfile: () -> Unit
) {
    when {
        body == null -> Unit
        !body.available -> Unit
        !body.connected -> item("body-connect") {
            StatsCard(c, title = "From your watch", index = 2) {
                ConnectWatchCard(onConnectWatch)
            }
        }
        else -> {
            val anything = listOfNotNull(body.sleepMin, body.restingHr, body.hrvMs, body.steps).any { it.hasData }
            if (!anything) {
                item("body-waiting") {
                    StatsCard(c, title = "From your watch", index = 2) {
                        Text(
                            "Connected, but nothing has synced in the last two weeks. Open your watch's app and let it sync to Health Connect.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = c.muted
                        )
                    }
                }
            }
            body.sleepMin?.takeIf { it.hasData }?.let { s ->
                item("body-sleep") {
                    SignalCard(
                        c, title = "Sleep", index = 2,
                        latestLabel = "Last night",
                        latest = s.latest?.let { formatHm(it.roundToInt()) },
                        usual = s.usual?.let { formatHm(it.roundToInt()) },
                        delta = deltaLine(s, unit = "") { formatHm(abs(it).roundToInt()) },
                        series = s, bars = true,
                        reading = "Sleep over two weeks" + (s.latest?.let { ", last night ${formatHm(it.roundToInt())}" } ?: "")
                    )
                }
            }
            body.restingHr?.takeIf { it.hasData }?.let { s ->
                item("body-rhr") {
                    SignalCard(
                        c, title = "Resting heart rate", index = 3,
                        latestLabel = "Today",
                        latest = s.latest?.let { "${it.roundToInt()} bpm" },
                        usual = s.usual?.let { "${it.roundToInt()} bpm" },
                        delta = deltaLine(s, unit = " bpm", tolerance = 2.0) { "${abs(it).roundToInt()}" },
                        caption = "Lower is fresher. A few beats over your usual often means you need more recovery.",
                        series = s, bars = false,
                        reading = "Resting heart rate" + (s.latest?.let { ", today ${it.roundToInt()} beats per minute" } ?: "")
                    )
                }
            }
            body.hrvMs?.takeIf { it.hasData }?.let { s ->
                item("body-hrv") {
                    SignalCard(
                        c, title = "Heart rate variability", index = 4,
                        latestLabel = "Today",
                        latest = s.latest?.let { "${it.roundToInt()} ms" },
                        usual = s.usual?.let { "${it.roundToInt()} ms" },
                        delta = deltaLine(s, unit = " ms", tolerance = (s.usual ?: 0.0) * 0.1) { "${abs(it).roundToInt()}" },
                        caption = "Higher is fresher. Read it against your own usual, never someone else's.",
                        series = s, bars = false,
                        reading = "Heart rate variability" + (s.latest?.let { ", today ${it.roundToInt()} milliseconds" } ?: "")
                    )
                }
            }
            body.steps?.takeIf { it.hasData }?.let { s ->
                item("body-steps") {
                    SignalCard(
                        c, title = "Steps", index = 5,
                        latestLabel = "Today",
                        latest = s.latest?.let { "%,d".format(it.roundToInt()) } ?: "0",
                        usual = s.usual?.let { "%,d".format(it.roundToInt()) },
                        usualLabel = "Typical",
                        delta = null,
                        series = s, bars = true,
                        reading = "Steps over two weeks"
                    )
                }
            }
        }
    }

    item("body-weight") {
        StatsCard(c, title = "Bodyweight", index = 6) {
            BodyweightContent(state.bodyweightPoints, weightUnit, c, onOpenProfile)
        }
    }
}

/** "39m under usual", "5 bpm over usual", or null inside [tolerance] or without a usual. */
private fun deltaLine(s: BodySeries, unit: String, tolerance: Double = 15.0, fmt: (Double) -> String): String? {
    val latest = s.latest ?: return null
    val usual = s.usual ?: return null
    val d = latest - usual
    if (abs(d) <= tolerance) return "About your usual"
    return "${fmt(d)}$unit ${if (d < 0) "under" else "over"} usual"
}

private fun formatHm(min: Int): String {
    val h = min / 60
    val m = min % 60
    return if (h == 0) "${m}m" else if (m == 0) "${h}h" else "${h}h ${m}m"
}

@Composable
private fun SignalCard(
    c: StatsColors,
    title: String,
    index: Int,
    latestLabel: String,
    latest: String?,
    usual: String?,
    delta: String?,
    series: BodySeries,
    bars: Boolean,
    reading: String,
    usualLabel: String = "Usual",
    caption: String? = null
) {
    StatsCard(c, title = title, caption = caption, index = index) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
            Column(Modifier.weight(1f)) {
                Text(latestLabel.uppercase(), style = MaterialTheme.typography.labelMedium, color = c.muted)
                Spacer(Modifier.height(2.dp))
                Text(latest ?: "Not yet", style = MaterialTheme.typography.headlineMedium, color = if (latest != null) c.onBg else c.muted)
                delta?.let {
                    Spacer(Modifier.height(2.dp))
                    Text(it, style = MaterialTheme.typography.bodySmall, color = c.muted)
                }
            }
            usual?.let {
                Column(horizontalAlignment = Alignment.End) {
                    Text(usualLabel.uppercase(), style = MaterialTheme.typography.labelMedium, color = c.muted)
                    Spacer(Modifier.height(2.dp))
                    Text(it, style = MaterialTheme.typography.titleMedium, color = c.onBg)
                }
            }
        }
        Spacer(Modifier.height(14.dp))
        DailyChart(series, bars, c, Modifier.fillMaxWidth().height(72.dp).clearAndSetSemantics { contentDescription = reading })
        Spacer(Modifier.height(6.dp))
        Row(Modifier.fillMaxWidth()) {
            Text("2 WEEKS AGO", style = MaterialTheme.typography.labelSmall, color = c.muted, modifier = Modifier.weight(1f))
            Text("TODAY", style = MaterialTheme.typography.labelSmall, color = c.muted)
        }
    }
}

/**
 * One value per day: bars (sleep, steps) or a line (heart signals), the newest in accent, and the
 * user's usual as a dashed rule. A missing day is a gap, never a zero.
 */
@Composable
private fun DailyChart(series: BodySeries, bars: Boolean, c: StatsColors, modifier: Modifier) {
    val values = series.days
    val present = values.filterNotNull()
    if (present.isEmpty()) return
    val usual = series.usual
    val hi = (present + listOfNotNull(usual)).max()
    val lo = if (bars) 0.0 else (present + listOfNotNull(usual)).min()
    val span = (hi - lo).takeIf { it > 0 } ?: 1.0
    val faint = c.onBg.copy(alpha = 0.25f)
    Canvas(modifier) {
        val n = values.size
        val slot = size.width / n
        // Lines get headroom top and bottom so the stroke and dots do not clip.
        val pad = if (bars) 0f else 6.dp.toPx()
        fun y(v: Double) = pad + (size.height - 2 * pad) * (1f - ((v - lo) / span).toFloat())
        if (bars) {
            val w = slot * 0.6f
            values.forEachIndexed { i, v ->
                if (v == null) return@forEachIndexed
                val top = y(v)
                drawRoundRect(
                    color = if (i == n - 1) c.accent else faint,
                    topLeft = Offset(i * slot + (slot - w) / 2, top),
                    size = Size(w, size.height - top),
                    cornerRadius = CornerRadius(3.dp.toPx())
                )
            }
        } else {
            val path = Path()
            var started = false
            values.forEachIndexed { i, v ->
                if (v == null) { started = false; return@forEachIndexed }
                val p = Offset(i * slot + slot / 2, y(v))
                if (started) path.lineTo(p.x, p.y) else path.moveTo(p.x, p.y)
                started = true
            }
            drawPath(path, color = c.onBg.copy(alpha = 0.6f), style = Stroke(width = 2.dp.toPx()))
            values.forEachIndexed { i, v ->
                if (v == null) return@forEachIndexed
                val p = Offset(i * slot + slot / 2, y(v))
                drawCircle(if (i == n - 1) c.accent else c.onBg.copy(alpha = 0.6f), radius = if (i == n - 1) 4.dp.toPx() else 2.dp.toPx(), center = p)
            }
        }
        usual?.let { u ->
            val uy = y(u)
            drawLine(
                color = c.muted.copy(alpha = 0.6f),
                start = Offset(0f, uy),
                end = Offset(size.width, uy),
                strokeWidth = 1.dp.toPx(),
                pathEffect = PathEffect.dashPathEffect(floatArrayOf(4.dp.toPx(), 4.dp.toPx()))
            )
        }
    }
}

@Composable
private fun BodyweightContent(
    points: List<com.forge.app.ui.gym.stats.state.BodyweightPoint>,
    weightUnit: WeightUnit,
    c: StatsColors,
    onOpenProfile: () -> Unit
) {
    val sorted = points.sortedBy { it.recordedAt }
    val latest = sorted.lastOrNull()
    if (latest == null) {
        Text(
            "No weigh-ins yet. Log one on your profile, or connect a smart scale through Health Connect.",
            style = MaterialTheme.typography.bodyMedium,
            color = c.muted
        )
    } else {
        val monthAgo = latest.recordedAt - 30L * 24 * 60 * 60 * 1000
        val base = sorted.lastOrNull { it.recordedAt <= monthAgo } ?: sorted.first()
        val change = toDisplayWeight(latest.weightLb - base.weightLb, weightUnit)
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
            Column(Modifier.weight(1f)) {
                Text("LATEST", style = MaterialTheme.typography.labelMedium, color = c.muted)
                Spacer(Modifier.height(2.dp))
                Text(formatWeight(latest.weightLb, weightUnit), style = MaterialTheme.typography.headlineMedium, color = c.onBg)
            }
            if (base !== latest) {
                Column(horizontalAlignment = Alignment.End) {
                    Text("30 DAYS", style = MaterialTheme.typography.labelMedium, color = c.muted)
                    Spacer(Modifier.height(2.dp))
                    Text(
                        (if (change >= 0) "+" else "−") + "%.1f %s".format(abs(change), weightUnit.label),
                        style = MaterialTheme.typography.titleMedium,
                        color = c.onBg
                    )
                }
            }
        }
        if (sorted.size >= 2) {
            Spacer(Modifier.height(14.dp))
            val values = sorted.takeLast(30).map { it.weightLb }
            com.forge.app.ui.gym.stats.components.Sparkline(
                values = values,
                lineColor = c.accent,
                minValue = values.min(),
                maxValue = values.max(),
                modifier = Modifier.fillMaxWidth().height(72.dp)
                    .clearAndSetSemantics { contentDescription = "Bodyweight trend, latest ${formatWeight(latest.weightLb, weightUnit)}" }
            )
        }
    }
    Spacer(Modifier.height(12.dp))
    Text(
        "Log a weigh-in →",
        style = MaterialTheme.typography.labelLarge,
        color = c.accent,
        modifier = Modifier
            .clickableLabeled("Open your profile to log a weigh-in", onClick = onOpenProfile)
            .padding(vertical = 8.dp)
    )
}
