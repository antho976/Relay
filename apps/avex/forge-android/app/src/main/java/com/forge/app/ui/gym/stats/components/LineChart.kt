package com.forge.app.ui.gym.stats.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color

/**
 * Full-size line chart over an ordered series — [Sparkline] dressed for the full-width charts:
 * top/middle/baseline gridlines, a single end-point marker, an optional OLS [trendColor] overlay, and
 * a near-zero range floor. Presentation-only: callers pre-convert to display units and supply the band.
 * Kept as a thin wrapper so there's exactly one line/reveal/trend implementation (in [Sparkline]).
 */
@Composable
internal fun LineChart(
    values: List<Double>,
    lineColor: Color,
    modifier: Modifier = Modifier,
    trendColor: Color? = null,
    minValue: Double = values.minOrNull() ?: 0.0,
    maxValue: Double = values.maxOrNull() ?: 1.0,
    /** 0→1 left-to-right reveal (see rememberDrawProgress). 1f = fully drawn. */
    progress: Float = 1f
) {
    Sparkline(
        values = values,
        lineColor = lineColor,
        minValue = minValue,
        maxValue = maxValue,
        modifier = modifier,
        progress = progress,
        gridFractions = listOf(0f, 0.5f, 1f),
        showStartEndpoint = false,
        trendColor = trendColor,
        rangeFloor = 1e-6
    )
}
