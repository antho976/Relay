package com.forge.app.ui.cardio.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** The geometry of one bar in a [VerticalBarRow] — its height and how it's painted. */
internal data class BarGeom(
    val height: Dp,
    val fill: Color = Color.Transparent,
    /** When non-null, the bar is drawn as a dashed outline (an empty/placeholder slot) not a fill. */
    val dashedOutline: Color? = null
)

/**
 * A row of equal-width vertical bars, each in a fixed-height bottom-aligned track, with optional
 * caller-drawn labels above/below via the [top]/[bottom] slots. The shared geometry behind the cardio
 * week row, the per-day stats bars, the empty-week scaffold and the hourly-steps graph — each keeps
 * its own labels/colours through the slots while sharing this layout (so the bar math lives in one place).
 */
@Composable
internal fun VerticalBarRow(
    count: Int,
    trackHeight: Dp,
    bar: (Int) -> BarGeom,
    modifier: Modifier = Modifier,
    spacing: Dp = 6.dp,
    labelSpacing: Dp = 4.dp,
    corner: Dp = 4.dp,
    top: (@Composable (Int) -> Unit)? = null,
    bottom: (@Composable (Int) -> Unit)? = null
) {
    Row(
        modifier = modifier,
        horizontalArrangement = Arrangement.spacedBy(spacing),
        verticalAlignment = Alignment.Bottom
    ) {
        repeat(count) { i ->
            val g = bar(i)
            Column(
                modifier = Modifier.weight(1f),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(labelSpacing)
            ) {
                top?.invoke(i)
                Box(Modifier.fillMaxWidth().height(trackHeight), contentAlignment = Alignment.BottomCenter) {
                    Box(
                        Modifier
                            .fillMaxWidth()
                            .height(g.height)
                            .clip(RoundedCornerShape(corner))
                            .then(
                                if (g.dashedOutline != null) {
                                    Modifier.drawBehind {
                                        drawRoundRect(
                                            color = g.dashedOutline,
                                            style = Stroke(
                                                width = 1.5.dp.toPx(),
                                                pathEffect = PathEffect.dashPathEffect(floatArrayOf(5f, 4f), 0f)
                                            ),
                                            cornerRadius = CornerRadius(corner.toPx())
                                        )
                                    }
                                } else {
                                    Modifier.background(g.fill)
                                }
                            )
                    )
                }
                bottom?.invoke(i)
            }
        }
    }
}
