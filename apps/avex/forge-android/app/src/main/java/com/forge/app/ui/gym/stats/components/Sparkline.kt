package com.forge.app.ui.gym.stats.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.foundation.Canvas
import androidx.compose.ui.unit.dp

@Composable
internal fun Sparkline(
    values: List<Double>,
    lineColor: Color,
    minValue: Double,
    maxValue: Double,
    modifier: Modifier = Modifier,
    /** 0→1 left-to-right reveal (see rememberDrawProgress). 1f = fully drawn. */
    progress: Float = 1f
) {
    if (values.size < 2) return
    val range = (maxValue - minValue).coerceAtLeast(1.0)
    val gridColor = lineColor.copy(alpha = 0.12f)

    Canvas(modifier = modifier) {
        val dashPx = 4.dp.toPx()
        val gridEffect = PathEffect.dashPathEffect(floatArrayOf(dashPx, dashPx))

        listOf(0.25f, 0.50f, 0.75f).forEach { frac ->
            drawLine(
                color = gridColor,
                start = Offset(0f, size.height * frac),
                end = Offset(size.width, size.height * frac),
                strokeWidth = 1.dp.toPx(),
                pathEffect = gridEffect
            )
        }

        // Gridlines render in full; the data line + endpoints reveal left-to-right.
        clipRect(right = size.width * progress.coerceIn(0f, 1f)) {
            val stepX = size.width / (values.size - 1)
            val strokeWidthPx = 2.dp.toPx()
            val path = Path()
            values.forEachIndexed { i, value ->
                val x = stepX * i
                val y = size.height - ((value - minValue) / range * size.height).toFloat()
                if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
            }
            drawPath(path = path, color = lineColor, style = Stroke(width = strokeWidthPx))

            val firstX = 0f
            val firstY = size.height - ((values.first() - minValue) / range * size.height).toFloat()
            val lastX = stepX * (values.size - 1)
            val lastY = size.height - ((values.last() - minValue) / range * size.height).toFloat()
            drawCircle(color = lineColor.copy(alpha = 0.5f), radius = 3.dp.toPx(), center = Offset(firstX, firstY))
            drawCircle(color = lineColor, radius = 4.dp.toPx(), center = Offset(lastX, lastY))
        }
    }
}
