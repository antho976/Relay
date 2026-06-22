package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/**
 * A horizontal progress bar with an optional target tick — the "am I doing enough" primitive for the
 * sets-per-muscle read. [fraction] and [targetFraction] are 0→1 against a shared max so a row of bars
 * is comparable. The tick stands proud of the bar so the target reads even when the fill overshoots.
 */
@Composable
internal fun TargetBar(
    fraction: Float,
    targetFraction: Float?,
    fillColor: Color,
    trackColor: Color,
    tickColor: Color,
    modifier: Modifier = Modifier,
    progress: Float = 1f
) {
    Canvas(modifier) {
        val h = size.height
        val radius = CornerRadius(h / 2, h / 2)
        drawRoundRect(color = trackColor, size = Size(size.width, h), cornerRadius = radius)
        val w = (size.width * fraction.coerceIn(0f, 1f) * progress)
        if (w > 0f) {
            drawRoundRect(color = fillColor, size = Size(w.coerceAtLeast(h), h), cornerRadius = radius)
        }
        targetFraction?.let { tf ->
            val x = (size.width * tf.coerceIn(0f, 1f)).coerceIn(0f, size.width)
            drawLine(
                color = tickColor,
                start = Offset(x, -2.dp.toPx()),
                end = Offset(x, h + 2.dp.toPx()),
                strokeWidth = 2.dp.toPx()
            )
        }
    }
}

/**
 * A center-anchored diverging bar for a paired ratio (push/pull, quad/ham). The two sides grow out
 * from the midline, each scaled to the larger count, so asymmetry = imbalance at a glance — the
 * honest read the spec wants instead of a radar. A faint midline marks the center.
 */
@Composable
internal fun DivergingBar(
    leftValue: Int,
    rightValue: Int,
    leftColor: Color,
    rightColor: Color,
    midlineColor: Color,
    modifier: Modifier = Modifier,
    progress: Float = 1f
) {
    Canvas(modifier) {
        val h = size.height
        val half = size.width / 2f
        val maxSide = maxOf(leftValue, rightValue, 1)
        val radius = CornerRadius(h / 2, h / 2)
        val p = progress.coerceIn(0f, 1f)

        val leftW = half * (leftValue.toFloat() / maxSide) * p
        if (leftW > 0f) {
            drawRoundRect(
                color = leftColor,
                topLeft = Offset(half - leftW, 0f),
                size = Size(leftW, h),
                cornerRadius = radius
            )
        }
        val rightW = half * (rightValue.toFloat() / maxSide) * p
        if (rightW > 0f) {
            drawRoundRect(
                color = rightColor,
                topLeft = Offset(half, 0f),
                size = Size(rightW, h),
                cornerRadius = radius
            )
        }
        drawLine(
            color = midlineColor,
            start = Offset(half, -1.dp.toPx()),
            end = Offset(half, h + 1.dp.toPx()),
            strokeWidth = 1.5.dp.toPx()
        )
    }
}

/**
 * A horizontal bar split into colored tier zones with a marker sitting on it — the relative-strength
 * "where do I rank" read. [zoneColors] paints the bands left→right; [zoneEdges] are the cumulative
 * 0→1 boundaries between them (size == zoneColors.size - 1); [markerFraction] is the value's position.
 */
@Composable
internal fun BandedBar(
    markerFraction: Float,
    zoneEdges: List<Float>,
    zoneColors: List<Color>,
    markerColor: Color,
    modifier: Modifier = Modifier
) {
    Canvas(modifier) {
        val h = size.height
        val w = size.width
        val radius = CornerRadius(h / 2, h / 2)
        var startFrac = 0f
        zoneColors.forEachIndexed { i, color ->
            val endFrac = if (i < zoneEdges.size) zoneEdges[i] else 1f
            val x0 = (w * startFrac).coerceIn(0f, w)
            val x1 = (w * endFrac).coerceIn(0f, w)
            if (x1 > x0) drawRoundRect(
                color = color,
                topLeft = Offset(x0, 0f),
                size = Size(x1 - x0, h),
                cornerRadius = radius
            )
            startFrac = endFrac
        }
        val mx = (w * markerFraction.coerceIn(0f, 1f)).coerceIn(0f, w)
        drawLine(
            color = markerColor,
            start = Offset(mx, -3.dp.toPx()),
            end = Offset(mx, h + 3.dp.toPx()),
            strokeWidth = 2.5.dp.toPx()
        )
        drawCircle(color = markerColor, radius = h * 0.6f, center = Offset(mx, h / 2f))
    }
}
