package com.forge.app.ui.coach

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
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
 * A small smooth-curve sparkline: a thin accent line and one solid end marker, revealed left to
 * right. No area wash under it: at this size the fill read as a blur beneath the line and was the
 * most dated thing on the page. A flat series draws as a centred line.
 */
@Composable
internal fun CoachSparkline(
    values: List<Double>,
    accent: Color,
    pageBg: Color,
    modifier: Modifier = Modifier,
    height: androidx.compose.ui.unit.Dp = 56.dp,
    // Optional y-range for a caller that overlays its own reference line and must share the scale.
    domainMin: Double? = null,
    domainMax: Double? = null
) {
    if (values.size < 2) return
    val progress = rememberCoachDraw(values.size)
    val lo = minOf(values.min(), domainMin ?: values.min())
    val hi = maxOf(values.max(), domainMax ?: values.max())
    val pad = if (hi - lo < 1e-6) 1.0 else 0.0
    val minV = lo - pad
    val range = ((hi + pad) - minV).coerceAtLeast(1.0)
    Canvas(modifier = modifier.fillMaxWidth().height(height)) {
        val hInset = 6.dp.toPx()
        val vInset = 6.dp.toPx()
        val plotW = (size.width - hInset * 2).coerceAtLeast(1f)
        val plotH = (size.height - vInset * 2).coerceAtLeast(1f)
        val stepX = plotW / (values.size - 1)
        fun yOf(v: Double): Float {
            val t = ((v - minV) / range).toFloat().coerceIn(0f, 1f)
            return vInset + (1f - t) * plotH
        }
        val pts = values.mapIndexed { i, v -> Offset(hInset + stepX * i, yOf(v)) }
        val line = coachSmoothCurve(pts, minY = vInset, maxY = size.height - vInset)
        val clip = (size.width * progress.coerceIn(0f, 1f)).coerceAtLeast(0.01f)
        clipRect(right = clip) {
            drawPath(line, color = accent, style = Stroke(width = 1.75.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
            val last = pts.last()
            drawCircle(color = pageBg, radius = 4.5.dp.toPx(), center = last)
            drawCircle(color = accent, radius = 3.dp.toPx(), center = last)
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
 * The recovery load gauge: one thin continuous track with the score filled along it and the
 * deload line standing up out of it as a tick. The scale runs a little PAST the line, so the line
 * reads as a mark on the way rather than as the end of the bar; the fill turns error once it
 * crosses. It replaced a 10dp bar of one chunky segment per point, which read as a loading widget.
 */
@Composable
internal fun CoachFatigueMeter(score: Int, threshold: Int, c: CoachColors) {
    val line = threshold.coerceAtLeast(1)
    val total = maxOf(line + 3, score + 1)
    val progress = rememberCoachDraw("fatigue-$score")
    val fillColor = if (score >= line) c.error else c.accent
    Column {
        Canvas(
            Modifier.fillMaxWidth().height(14.dp).semantics {
                contentDescription = "Recovery load $score, deload at $line"
            }
        ) {
            val track = 3.dp.toPx()
            val cy = size.height / 2f
            val tickX = size.width * line / total
            drawLine(c.track, Offset(0f, cy), Offset(size.width, cy), track, StrokeCap.Round)
            val fillTo = size.width * (score.toFloat() / total) * progress
            if (fillTo > 0f) drawLine(fillColor, Offset(0f, cy), Offset(fillTo, cy), track, StrokeCap.Round)
            drawLine(c.onBg, Offset(tickX, 0f), Offset(tickX, size.height), 1.5.dp.toPx(), StrokeCap.Round)
        }
        Spacer(Modifier.height(6.dp))
        // The caption hangs under the tick it names, so the number and the line are one object.
        androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxWidth()) {
            val tickFrac = line.toFloat() / total
            Text(
                "DELOAD AT $line",
                style = MaterialTheme.typography.labelSmall,
                color = c.muted,
                fontSize = 9.sp,
                letterSpacing = 1.sp,
                modifier = Modifier.align(Alignment.TopEnd)
                    .padding(end = (maxWidth * (1f - tickFrac) - 2.dp).coerceAtLeast(0.dp))
            )
        }
    }
}

/**
 * The zero-shape of [CoachSparkline] (§12): a flat ghost line with a hollow end marker, drawn
 * on the outline-border rung (0.35 — the 0.25 hairline vanishes at this stroke weight on
 * near-black). Stands in for a lift whose trend hasn't formed yet.
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

/** A thin countdown bar: full-strength fill on the outline track rung. */
@Composable
internal fun CoachWatchBar(
    fraction: Float,
    color: Color,
    c: CoachColors,
    modifier: Modifier = Modifier
) {
    val progress = rememberCoachDraw("watch-$fraction")
    Box(
        modifier
            .fillMaxWidth()
            .height(5.dp)
            .clip(RoundedCornerShape(50))
            .background(c.track)
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
 * Nightly sleep as thin rounded columns against the recovery floor, dashed across them. Rested
 * nights take the accent, short ones the 0.6 rung. Columns at the chart doctrine's comparison-bar
 * weight, not the slabs they were: ten fat red blocks were the loudest thing on the page.
 */
@Composable
internal fun CoachSleepBars(hours: List<Float>, floorHours: Float, c: CoachColors) {
    if (hours.isEmpty()) return
    val max = maxOf(hours.max(), floorHours + 1f)
    val progress = rememberCoachDraw(hours.size)
    val floorText = String.format(java.util.Locale.US, "%.1f", floorHours)
    Column {
        Canvas(Modifier.fillMaxWidth().height(56.dp)) {
            val slot = size.width / hours.size
            val bar = minOf(6.dp.toPx(), slot * 0.5f)
            hours.forEachIndexed { i, h ->
                val frac = ((h / max) * progress).coerceIn(0.04f, 1f)
                val cx = slot * i + slot / 2f
                drawLine(
                    color = if (h >= floorHours) c.accent else c.secondary,
                    start = Offset(cx, size.height - bar / 2f),
                    end = Offset(cx, (size.height - size.height * frac + bar / 2f).coerceAtMost(size.height - bar / 2f)),
                    strokeWidth = bar,
                    cap = StrokeCap.Round
                )
            }
            val y = size.height * (1f - (floorHours / max))
            drawLine(
                color = c.muted.copy(alpha = 0.6f),
                start = Offset(0f, y),
                end = Offset(size.width, y),
                strokeWidth = 1.dp.toPx(),
                pathEffect = androidx.compose.ui.graphics.PathEffect.dashPathEffect(floatArrayOf(4.dp.toPx(), 4.dp.toPx()))
            )
        }
        Spacer(Modifier.height(8.dp))
        Text(
            "LAST ${hours.size} NIGHTS · FLOOR ${floorText}H",
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
            // One range for the line and the baseline, or the same y would mean different readings.
            val lo = minOf(values.min(), baseline ?: values.min()).toFloat()
            val hi = maxOf(values.max(), baseline ?: values.max()).toFloat()
            CoachSparkline(
                values.map { it.toDouble() }, c.accent, c.bg, height = 56.dp,
                domainMin = lo.toDouble(), domainMax = hi.toDouble()
            )
            if (baseline != null) {
                Canvas(Modifier.fillMaxWidth().height(56.dp)) {
                    val vInset = 6.dp.toPx()
                    val plotH = size.height - vInset * 2
                    val t = if (hi - lo < 1e-6f) 0.5f else (baseline - lo) / (hi - lo)
                    val y = vInset + (1f - t) * plotH
                    drawLine(
                        color = c.muted.copy(alpha = 0.6f),
                        start = Offset(0f, y),
                        end = Offset(size.width, y),
                        strokeWidth = 1.dp.toPx(),
                        pathEffect = androidx.compose.ui.graphics.PathEffect.dashPathEffect(floatArrayOf(4.dp.toPx(), 4.dp.toPx()))
                    )
                }
            }
        }
        Spacer(Modifier.height(8.dp))
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

