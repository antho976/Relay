package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

// TargetBar + DivergingBar removed 2026-07-01 — sets-per-muscle now sizes each track BY its target
// (fill the bar = hit the plan) and Balance draws a single split-ratio bar, both inline in
// StatsVolume. The old shared-scale bars with floating target ticks read as confusing stray lines.

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
