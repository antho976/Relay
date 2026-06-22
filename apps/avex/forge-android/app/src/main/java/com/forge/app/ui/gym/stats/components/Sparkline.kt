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

/**
 * Ordinary-least-squares fit over (index, value) points → (slope, intercept), or null when there's
 * nothing to fit. Index-based X: callers pass an ordered series and the trend is "per step".
 */
internal fun olsTrend(values: List<Double>): Pair<Double, Double>? {
    val n = values.size
    if (n < 2) return null
    val meanX = (n - 1) / 2.0
    val meanY = values.average()
    var num = 0.0
    var den = 0.0
    for (i in 0 until n) {
        val dx = i - meanX
        num += dx * (values[i] - meanY)
        den += dx * dx
    }
    if (den == 0.0) return null
    val slope = num / den
    return slope to (meanY - slope * meanX)
}

/**
 * The one line-series chart: an ordered series drawn with a left-to-right reveal, dashed gridlines, and
 * endpoint markers. The small inline form is the default; [LineChart] is this same composable wearing
 * the full-size dress (full-bleed grid, single end marker, optional trend) so the line geometry, reveal,
 * and trend math have a single definition. X is the point ordinal; Y scales to [[minValue], [maxValue]].
 */
@Composable
internal fun Sparkline(
    values: List<Double>,
    lineColor: Color,
    minValue: Double,
    maxValue: Double,
    modifier: Modifier = Modifier,
    /** 0→1 left-to-right reveal (see rememberDrawProgress). 1f = fully drawn. */
    progress: Float = 1f,
    /** Vertical positions of the dashed guide lines, 0 (top) → 1 (bottom). */
    gridFractions: List<Float> = listOf(0.25f, 0.50f, 0.75f),
    /** Draw the faint marker on the first point (off for the full-size chart). */
    showStartEndpoint: Boolean = true,
    /** When non-null, an OLS trend line is drawn under the data line. */
    trendColor: Color? = null,
    /** Minimum span so an all-equal / near-flat series doesn't divide by ~0. */
    rangeFloor: Double = 1.0
) {
    if (values.size < 2) return
    val range = (maxValue - minValue).coerceAtLeast(rangeFloor)
    val gridColor = lineColor.copy(alpha = 0.12f)

    Canvas(modifier = modifier) {
        val dashPx = 4.dp.toPx()
        val gridEffect = PathEffect.dashPathEffect(floatArrayOf(dashPx, dashPx))

        // Gridlines render in full; the data line + endpoints reveal left-to-right.
        gridFractions.forEach { frac ->
            drawLine(
                color = gridColor,
                start = Offset(0f, size.height * frac),
                end = Offset(size.width, size.height * frac),
                strokeWidth = 1.dp.toPx(),
                pathEffect = gridEffect
            )
        }

        fun yOf(v: Double) = size.height - ((v - minValue) / range * size.height).toFloat()
        val clip = size.width * progress.coerceIn(0f, 1f)
        val stepX = size.width / (values.size - 1)

        // Trend first, so the data line reads on top of it.
        if (trendColor != null) {
            olsTrend(values)?.let { (slope, intercept) ->
                clipRect(right = clip) {
                    drawLine(
                        color = trendColor,
                        start = Offset(0f, yOf(intercept)),
                        end = Offset(size.width, yOf(intercept + slope * (values.size - 1))),
                        strokeWidth = 1.5.dp.toPx(),
                        pathEffect = gridEffect
                    )
                }
            }
        }

        clipRect(right = clip) {
            val path = Path()
            values.forEachIndexed { i, value ->
                val x = stepX * i
                val y = yOf(value)
                if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
            }
            drawPath(path = path, color = lineColor, style = Stroke(width = 2.dp.toPx()))

            if (showStartEndpoint) {
                drawCircle(color = lineColor.copy(alpha = 0.5f), radius = 3.dp.toPx(), center = Offset(0f, yOf(values.first())))
            }
            drawCircle(color = lineColor, radius = 4.dp.toPx(), center = Offset(stepX * (values.size - 1), yOf(values.last())))
        }
    }
}
