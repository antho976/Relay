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
 * A one-dimensional readiness gauge: the current fatigue [score] as a marker on a track split into
 * Fresh / Building / Deload bands, with the learned deload [threshold] drawn as a line. "The
 * threshold-as-a-line makes the abstract score legible." Bands mirror [buildReadinessPulse]: Building
 * is [threshold-2, threshold), Deload is ≥ threshold.
 *
 * A fatigue line *over time* is the eventual form (needs a per-week fatigue history the engine doesn't
 * expose yet); this gauge is the legible v1 of the same decision signal.
 */
@Composable
internal fun FatigueGauge(
    score: Int,
    threshold: Int,
    freshColor: Color,
    buildingColor: Color,
    deloadColor: Color,
    markerColor: Color,
    modifier: Modifier = Modifier
) {
    val maxScale = maxOf(threshold + 4, score + 2, 8).toFloat()
    val buildingStart = (threshold - 2).coerceAtLeast(0) / maxScale
    val deloadStart = threshold / maxScale
    val scoreFrac = (score / maxScale).coerceIn(0f, 1f)

    Canvas(modifier) {
        val h = size.height
        val w = size.width
        val barH = h * 0.5f
        val top = (h - barH) / 2f
        val radius = CornerRadius(barH / 2, barH / 2)

        fun band(from: Float, to: Float, color: Color) {
            val x0 = (w * from).coerceIn(0f, w)
            val x1 = (w * to).coerceIn(0f, w)
            if (x1 > x0) drawRoundRect(
                color = color,
                topLeft = Offset(x0, top),
                size = Size(x1 - x0, barH),
                cornerRadius = radius
            )
        }
        band(0f, buildingStart, freshColor)
        band(buildingStart, deloadStart, buildingColor)
        band(deloadStart, 1f, deloadColor)

        // Threshold line — the deload gate.
        val tx = (w * deloadStart).coerceIn(0f, w)
        drawLine(
            color = markerColor.copy(alpha = 0.7f),
            start = Offset(tx, top - 4.dp.toPx()),
            end = Offset(tx, top + barH + 4.dp.toPx()),
            strokeWidth = 1.5.dp.toPx()
        )

        // Current-score marker.
        val mx = (w * scoreFrac).coerceIn(0f, w)
        drawCircle(color = markerColor, radius = barH * 0.45f, center = Offset(mx, top + barH / 2f))
        drawCircle(color = Color.White, radius = barH * 0.18f, center = Offset(mx, top + barH / 2f))
    }
}
