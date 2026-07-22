package com.forge.app.ui.profile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.ui.gym.stats.components.rememberDrawProgress
import com.forge.app.ui.theme.ForgeMotion

/**
 * Profile-local mini chart primitives. Deliberately self-contained (no dependency on the stats
 * `components` package) so the two screens can evolve independently — the profile tiles only need a
 * tiny line and a progress ring, not the full stats chart kit.
 */

/**
 * A small line sparkline with a soft gradient fill and an end dot. Draws nothing below 2 points.
 * When [animated], the line wipes in left-to-right on first appearance (one-shot, re-keyed by the
 * series) with the end dot riding the reveal frontier — the "graph slides in" entrance.
 *
 * [values] is the main line (e.g. a smoothed trend). [points] draws faint scatter dots around it
 * (the raw series, index-aligned to [values]) in [pointColor] — subordinate to the trend line.
 * [reference] draws a dashed horizontal threshold (a goal/target) in [referenceColor]. The y-range
 * spans all three, so the raw scatter and a goal beyond every value still stay on-canvas.
 */
@Composable
internal fun ProfileSparkline(
    values: List<Double>,
    color: Color,
    modifier: Modifier = Modifier,
    fill: Boolean = true,
    animated: Boolean = true,
    points: List<Double>? = null,
    pointColor: Color = color,
    reference: Double? = null,
    referenceColor: Color = color.copy(alpha = 0.6f)
) {
    if (values.size < 2) return
    val progress = if (animated) rememberDrawProgress(key = values, spec = ForgeMotion.drawTween()) else 1f
    // Range spans the line, the raw scatter (dots sit outside the smoothed envelope) and the
    // reference (a goal can be beyond every weigh-in) so nothing is drawn off-canvas.
    val min = minOf(values.min(), points?.minOrNull() ?: Double.POSITIVE_INFINITY, reference ?: Double.POSITIVE_INFINITY)
    val max = maxOf(values.max(), points?.maxOrNull() ?: Double.NEGATIVE_INFINITY, reference ?: Double.NEGATIVE_INFINITY)
    val range = (max - min).takeIf { it > 0.0 } ?: 1.0
    Canvas(modifier) {
        val h = size.height
        val w = size.width
        val stepX = if (values.size > 1) w / (values.size - 1) else 0f
        fun yOf(v: Double) = h - ((v - min) / range * h).toFloat()
        fun pointAt(i: Int): Offset = Offset(stepX * i, yOf(values[i]))
        // The point on the piecewise-linear curve at horizontal fraction [f] (0..1) — the reveal frontier.
        fun frontier(f: Float): Offset {
            val pos = (f * (values.size - 1)).coerceIn(0f, (values.size - 1).toFloat())
            val i = pos.toInt().coerceIn(0, values.size - 2)
            val a = pointAt(i); val b = pointAt(i + 1)
            val t = pos - i
            return Offset(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
        }
        val line = Path().apply {
            values.indices.forEach { i ->
                val p = pointAt(i)
                if (i == 0) moveTo(p.x, p.y) else lineTo(p.x, p.y)
            }
        }
        val revealW = (w * progress).coerceAtLeast(0.01f)
        clipRect(right = revealW) {
            // Reference (goal) threshold under the data, so the line + dots read on top of it.
            reference?.let { r ->
                val dashPx = 4.dp.toPx()
                drawLine(
                    referenceColor,
                    start = Offset(0f, yOf(r)),
                    end = Offset(w, yOf(r)),
                    strokeWidth = 1.5.dp.toPx(),
                    pathEffect = PathEffect.dashPathEffect(floatArrayOf(dashPx, dashPx))
                )
            }
            if (fill) {
                val area = Path().apply {
                    moveTo(0f, h)
                    values.indices.forEach { i -> val p = pointAt(i); lineTo(p.x, p.y) }
                    lineTo(w, h)
                    close()
                }
                drawPath(
                    area,
                    Brush.verticalGradient(listOf(color.copy(alpha = 0.15f), Color.Transparent))
                )
            }
            drawPath(line, color, style = Stroke(width = 2.dp.toPx(), cap = StrokeCap.Round))
            // Raw weigh-ins as faint scatter dots (index-aligned to [values]).
            points?.forEachIndexed { i, v -> drawCircle(pointColor, radius = 2.dp.toPx(), center = Offset(stepX * i, yOf(v))) }
        }
        drawCircle(color, radius = 3.dp.toPx(), center = frontier(progress))
    }
}

/**
 * A circular progress ring (full 360° track + an arc that sweeps from 12 o'clock). [content] is
 * centered inside the ring — typically the percent figure. When [animated], the arc sweeps up from
 * zero on first appearance (one-shot); the centered content shows its final value immediately.
 */
@Composable
internal fun ProgressRing(
    fraction: Float,
    color: Color,
    trackColor: Color,
    modifier: Modifier = Modifier,
    stroke: Dp = 5.dp,
    animated: Boolean = false,
    content: @Composable BoxScope.() -> Unit = {}
) {
    val sweep = if (animated) rememberDrawProgress(spec = ForgeMotion.drawTween()) else 1f
    val f = fraction.coerceIn(0f, 1f) * sweep
    Box(modifier, contentAlignment = Alignment.Center) {
        Canvas(Modifier.matchParentSize()) {
            val sw = stroke.toPx()
            val inset = sw / 2f
            val arcSize = Size(size.width - sw, size.height - sw)
            drawArc(
                color = trackColor,
                startAngle = -90f,
                sweepAngle = 360f,
                useCenter = false,
                topLeft = Offset(inset, inset),
                size = arcSize,
                style = Stroke(width = sw, cap = StrokeCap.Round)
            )
            if (f > 0f) drawArc(
                color = color,
                startAngle = -90f,
                sweepAngle = 360f * f,
                useCenter = false,
                topLeft = Offset(inset, inset),
                size = arcSize,
                style = Stroke(width = sw, cap = StrokeCap.Round)
            )
        }
        content()
    }
}
