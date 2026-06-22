package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp

/**
 * A scatter of raw [points] with an [overlay] line/curve drawn through them — the shared primitive for
 * "noisy dots + the honest smoothed read": bodyweight scatter + moving average (Tier 5) and the
 * load-rep set scatter + fitted strength curve (Tier 6). Both axes are in data units; the caller
 * supplies the bounds. [highlightOverlayEnd] marks the curve's last point (e.g. the extrapolated 1RM).
 */
@Composable
internal fun ScatterChart(
    points: List<Offset>,
    overlay: List<Offset>,
    pointColor: Color,
    lineColor: Color,
    gridColor: Color,
    minX: Float,
    maxX: Float,
    minY: Float,
    maxY: Float,
    modifier: Modifier = Modifier,
    highlightOverlayEnd: Boolean = false
) {
    val rangeX = (maxX - minX).coerceAtLeast(1e-3f)
    val rangeY = (maxY - minY).coerceAtLeast(1e-3f)
    Canvas(modifier) {
        val dashPx = 4.dp.toPx()
        val gridEffect = PathEffect.dashPathEffect(floatArrayOf(dashPx, dashPx))
        listOf(0f, 0.5f, 1f).forEach { frac ->
            drawLine(
                color = gridColor,
                start = Offset(0f, size.height * frac),
                end = Offset(size.width, size.height * frac),
                strokeWidth = 1.dp.toPx(),
                pathEffect = gridEffect
            )
        }

        fun px(x: Float, y: Float) = Offset(
            (x - minX) / rangeX * size.width,
            size.height - (y - minY) / rangeY * size.height
        )

        if (overlay.size >= 2) {
            val path = Path()
            overlay.forEachIndexed { i, o ->
                val p = px(o.x, o.y)
                if (i == 0) path.moveTo(p.x, p.y) else path.lineTo(p.x, p.y)
            }
            drawPath(path, color = lineColor, style = Stroke(width = 2.dp.toPx()))
            if (highlightOverlayEnd) {
                val last = overlay.last()
                drawCircle(lineColor, radius = 5.dp.toPx(), center = px(last.x, last.y))
            }
        }

        points.forEach { o ->
            drawCircle(pointColor.copy(alpha = 0.6f), radius = 3.dp.toPx(), center = px(o.x, o.y))
        }
    }
}
