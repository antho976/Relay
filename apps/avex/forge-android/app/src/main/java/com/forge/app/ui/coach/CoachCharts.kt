package com.forge.app.ui.coach

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.theme.ForgeMotion

/** Play-once draw progress for the page's charts, riding the shared slow draw curve. */
@Composable
internal fun rememberCoachDraw(key: Any = Unit): Float {
    var played by rememberSaveable(key) { mutableStateOf(false) }
    val anim = remember(key) { Animatable(if (played) 1f else 0f) }
    LaunchedEffect(key) {
        if (!played) {
            anim.animateTo(1f, ForgeMotion.drawTween())
            played = true
        }
    }
    return anim.value
}

/**
 * A small smooth-curve sparkline (the session-detail line idiom, sized for inline deep dives):
 * accent stroke over a soft gradient fill, page-background ring markers on the endpoints, and a
 * left-to-right reveal. A flat series draws as a centred line.
 */
@Composable
internal fun CoachSparkline(
    values: List<Double>,
    accent: Color,
    pageBg: Color,
    modifier: Modifier = Modifier,
    height: androidx.compose.ui.unit.Dp = 56.dp
) {
    if (values.size < 2) return
    val progress = rememberCoachDraw(values.size)
    val lo = values.min()
    val hi = values.max()
    val pad = if (hi - lo < 1e-6) 1.0 else 0.0
    val minV = lo - pad
    val range = ((hi + pad) - minV).coerceAtLeast(1.0)
    Canvas(modifier = modifier.fillMaxWidth().height(height)) {
        val hInset = 8.dp.toPx()
        val vInset = 8.dp.toPx()
        val plotW = (size.width - hInset * 2).coerceAtLeast(1f)
        val plotH = (size.height - vInset * 2).coerceAtLeast(1f)
        val stepX = plotW / (values.size - 1)
        fun yOf(v: Double): Float {
            val t = ((v - minV) / range).toFloat().coerceIn(0f, 1f)
            return vInset + (1f - t) * plotH
        }
        val pts = values.mapIndexed { i, v -> Offset(hInset + stepX * i, yOf(v)) }
        val baseline = size.height - vInset
        val line = coachSmoothCurve(pts, minY = vInset, maxY = baseline)
        val fill = Path().apply {
            addPath(line)
            lineTo(pts.last().x, baseline)
            lineTo(pts.first().x, baseline)
            close()
        }
        val clip = (size.width * progress.coerceIn(0f, 1f)).coerceAtLeast(0.01f)
        clipRect(right = clip) {
            // A wash, never a block — the fill sits at ~10% so the 2dp line stays the mark.
            drawPath(fill, brush = Brush.verticalGradient(listOf(accent.copy(alpha = 0.10f), accent.copy(alpha = 0f))))
            drawPath(line, color = accent, style = Stroke(width = 2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
            val last = pts.last()
            drawCircle(color = pageBg, radius = 5.5.dp.toPx(), center = last)
            drawCircle(color = accent, radius = 3.5.dp.toPx(), center = last)
            drawCircle(color = pageBg, radius = 1.3.dp.toPx(), center = last)
        }
    }
}

/** Catmull-Rom spline through [pts], control points clamped inside the plot box. */
private fun coachSmoothCurve(pts: List<Offset>, minY: Float, maxY: Float): Path {
    val path = Path()
    if (pts.isEmpty()) return path
    path.moveTo(pts.first().x, pts.first().y)
    if (pts.size < 3) {
        for (i in 1 until pts.size) path.lineTo(pts[i].x, pts[i].y)
        return path
    }
    for (i in 0 until pts.size - 1) {
        val p0 = pts[if (i == 0) 0 else i - 1]
        val p1 = pts[i]
        val p2 = pts[i + 1]
        val p3 = pts[if (i + 2 <= pts.lastIndex) i + 2 else pts.lastIndex]
        val c1x = p1.x + (p2.x - p0.x) / 6f
        val c1y = (p1.y + (p2.y - p0.y) / 6f).coerceIn(minY, maxY)
        val c2x = p2.x - (p3.x - p1.x) / 6f
        val c2y = (p2.y - (p3.y - p1.y) / 6f).coerceIn(minY, maxY)
        path.cubicTo(c1x, c1y, c2x, c2y, p2.x, p2.y)
    }
    return path
}

/**
 * The recovery load meter: the fatigue score as a segmented fill toward the deload line, one
 * segment per point. Fill carries severity (accent, error past the line) on the outline track
 * rung; labels stay in ink — the meter, not the text, carries the color.
 */
@Composable
internal fun CoachFatigueMeter(score: Int, threshold: Int, c: CoachColors) {
    val total = maxOf(threshold, score, 1)
    val progress = rememberCoachDraw("fatigue-$score")
    val fillColor = if (score >= threshold) c.error else c.accent
    Column {
        Row(
            Modifier.fillMaxWidth().height(10.dp).clip(RoundedCornerShape(5.dp)),
            horizontalArrangement = Arrangement.spacedBy(2.dp)
        ) {
            repeat(total) { i ->
                val filled = progress * score > i
                Box(
                    Modifier
                        .weight(1f)
                        .fillMaxHeight()
                        .background(if (filled) fillColor else c.outline.copy(alpha = 0.25f))
                )
            }
        }
        Spacer(Modifier.height(6.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(
                "NOW $score",
                style = MaterialTheme.typography.labelSmall,
                color = c.onBg,
                fontSize = 9.sp,
                letterSpacing = 1.sp
            )
            Text(
                "DELOAD AT $threshold",
                style = MaterialTheme.typography.labelSmall,
                color = c.muted,
                fontSize = 9.sp,
                letterSpacing = 1.sp
            )
        }
    }
}

/**
 * The zero-shape of [CoachSparkline] (§12): a flat ghost line with a hollow end marker, drawn
 * on the hairline rung. Stands in for a lift whose trend hasn't formed yet.
 */
@Composable
internal fun CoachGhostSpark(
    c: CoachColors,
    modifier: Modifier = Modifier,
    height: androidx.compose.ui.unit.Dp = 32.dp
) {
    val ghost = c.outline.copy(alpha = 0.35f)
    Canvas(modifier = modifier.height(height)) {
        val hInset = 8.dp.toPx()
        val y = size.height / 2f
        drawLine(
            color = ghost,
            start = Offset(hInset, y),
            end = Offset(size.width - hInset, y),
            strokeWidth = 2.dp.toPx(),
            cap = StrokeCap.Round
        )
        drawCircle(
            color = ghost,
            radius = 3.5.dp.toPx(),
            center = Offset(size.width - hInset, y),
            style = Stroke(width = 1.dp.toPx())
        )
    }
}

/** A thin countdown bar: full-strength fill on the outline track rung (§5). */
@Composable
internal fun CoachWatchBar(fraction: Float, color: Color, c: CoachColors) {
    val progress = rememberCoachDraw("watch-$fraction")
    Box(
        Modifier
            .fillMaxWidth()
            .height(5.dp)
            .clip(RoundedCornerShape(50))
            .background(c.outline.copy(alpha = 0.25f))
    ) {
        Box(
            Modifier
                .fillMaxWidth((fraction * progress).coerceIn(0.03f, 1f))
                .fillMaxHeight()
                .clip(RoundedCornerShape(50))
                .background(color)
        )
    }
}

/**
 * One dashboard progression row: two mono end labels over a bar. [segments] draws the earned
 * style segmented meter (n of m); [fraction] draws a continuous fill. The page's answer to
 * "how long until the next thing", drawn instead of written.
 */
@Composable
internal fun CoachProgressRow(
    label: String,
    value: String,
    c: CoachColors,
    modifier: Modifier = Modifier,
    fraction: Float? = null,
    segments: Pair<Int, Int>? = null,
    sub: String? = null,
    barColor: Color = c.accent
) {
    Column(modifier.fillMaxWidth().padding(bottom = 14.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(
                label.uppercase(), style = MaterialTheme.typography.labelSmall,
                color = c.muted, fontSize = 9.sp, letterSpacing = 1.sp
            )
            Text(
                value.uppercase(), style = MaterialTheme.typography.labelSmall,
                color = c.onBg, fontSize = 9.sp, letterSpacing = 1.sp
            )
        }
        Spacer(Modifier.height(6.dp))
        when {
            segments != null -> TrustProgressBar(
                streak = segments.first, required = segments.second,
                earned = segments.first >= segments.second,
                modifier = Modifier.fillMaxWidth()
            )
            fraction != null -> CoachWatchBar(fraction, barColor, c)
        }
        if (sub != null) {
            Spacer(Modifier.height(4.dp))
            Text(sub, style = MaterialTheme.typography.labelSmall, color = c.muted, fontSize = 9.sp)
        }
    }
}

/** Nightly sleep bars against the recovery floor; short nights render muted, rested ones accent. */
@Composable
internal fun CoachSleepBars(hours: List<Float>, floorHours: Float, c: CoachColors) {
    if (hours.isEmpty()) return
    val max = maxOf(hours.max(), floorHours + 1f)
    val progress = rememberCoachDraw(hours.size)
    Column {
        Box(Modifier.fillMaxWidth().height(64.dp)) {
            Row(
                Modifier.fillMaxSize(),
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                verticalAlignment = Alignment.Bottom
            ) {
                hours.forEach { h ->
                    val frac = ((h / max) * progress).coerceIn(0.04f, 1f)
                    // Two ladder rungs of the accent: rested nights full, short nights the 0.6 step.
                    Box(
                        Modifier
                            .weight(1f)
                            .fillMaxHeight(frac)
                            .clip(RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp))
                            .background(if (h >= floorHours) c.accent else c.secondary)
                    )
                }
            }
            // The floor line the sleep-debt driver measures against — a solid recessive hairline.
            Canvas(Modifier.fillMaxSize()) {
                val y = size.height * (1f - (floorHours / max))
                drawLine(
                    color = c.outline.copy(alpha = 0.35f),
                    start = Offset(0f, y),
                    end = Offset(size.width, y),
                    strokeWidth = 1.dp.toPx()
                )
            }
        }
        Spacer(Modifier.height(6.dp))
        Text(
            "LAST ${hours.size} NIGHTS · FLOOR ${String.format(java.util.Locale.US, "%.1f", floorHours)}H",
            style = MaterialTheme.typography.labelSmall,
            color = c.muted, fontSize = 9.sp, letterSpacing = 1.sp
        )
    }
}

/** Resting heart rate line with the prior-month baseline dashed underneath it. */
@Composable
internal fun CoachHrLine(values: List<Int>, baseline: Int?, c: CoachColors) {
    if (values.size < 2) return
    Column {
        Box {
            CoachSparkline(values.map { it.toDouble() }, c.accent, c.bg, height = 56.dp)
            if (baseline != null) {
                val lo = minOf(values.min(), baseline).toFloat()
                val hi = maxOf(values.max(), baseline).toFloat()
                Canvas(Modifier.fillMaxWidth().height(56.dp)) {
                    val vInset = 8.dp.toPx()
                    val plotH = size.height - vInset * 2
                    val t = if (hi - lo < 1e-6f) 0.5f else (baseline - lo) / (hi - lo)
                    val y = vInset + (1f - t) * plotH
                    drawLine(
                        color = c.outline.copy(alpha = 0.35f),
                        start = Offset(0f, y),
                        end = Offset(size.width, y),
                        strokeWidth = 1.dp.toPx()
                    )
                }
            }
        }
        Spacer(Modifier.height(6.dp))
        Text(
            buildString {
                append("NOW ${values.last()} BPM")
                if (baseline != null) append(" · BASELINE $baseline")
            },
            style = MaterialTheme.typography.labelSmall,
            color = c.muted, fontSize = 9.sp, letterSpacing = 1.sp
        )
    }
}

