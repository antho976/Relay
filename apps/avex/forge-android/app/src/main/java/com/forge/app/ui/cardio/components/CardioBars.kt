package com.forge.app.ui.cardio.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

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

/**
 * The thin bar a value fills toward a target, with its mono caption underneath (§2② — one value vs
 * a target is a meter). Cardio's one meter shape: the hero's weekly-minutes goal, the WHO reference
 * that stands in when no personal target is set, and a week row's standing in the ledger. At zero it
 * is an empty track, never a hidden section (§12).
 */
@Composable
internal fun MeterBar(
    fraction: Float,
    caption: String,
    muted: Color,
    outline: Color,
    accent: Color,
    modifier: Modifier = Modifier,
    /** Reads the meter's VALUE for TalkBack (§14), not its shape. Defaults to the caption. */
    contentDescription: String = caption
) {
    val frac = fraction.coerceIn(0f, 1f)
    Column(modifier.semantics(mergeDescendants = true) { this.contentDescription = contentDescription }) {
        Box(
            Modifier.fillMaxWidth().height(4.dp)
                .clip(RoundedCornerShape(4.dp))
                .background(outline.copy(alpha = 0.25f))
        ) {
            if (frac > 0f) {
                Box(Modifier.fillMaxWidth(frac).height(4.dp).clip(RoundedCornerShape(4.dp)).background(accent))
            }
        }
        Spacer(Modifier.height(6.dp))
        Text(
            caption.uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = muted, letterSpacing = 1.sp
        )
    }
}

/**
 * One row of a ranked comparison — a name, the thin bar its value fills against the leader, and the
 * value itself (§2②: ranked comparison = thin bars). The shape behind BY ACTIVITY and the session
 * detail's standing-against-your-best mark, so a ranked read looks the same wherever cardio draws one.
 */
@Composable
internal fun RankedBarRow(
    label: String,
    value: String,
    fraction: Float,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color,
    modifier: Modifier = Modifier,
    /** Draws this row's bar muted rather than accent — for a reference row beside the live one. */
    dimmed: Boolean = false
) {
    val frac = fraction.coerceIn(0f, 1f)
    Column(
        modifier
            .fillMaxWidth()
            .semantics(mergeDescendants = true) { contentDescription = "$label, $value" }
    ) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            // Sans row title against mono meta — the same pairing every cardio list row uses.
            Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg, modifier = Modifier.weight(1f))
            Spacer(Modifier.width(12.dp))
            Text(
                value.uppercase(),
                style = MaterialTheme.typography.labelMedium,
                color = muted
            )
        }
        Spacer(Modifier.height(8.dp))
        Box(
            Modifier.fillMaxWidth().height(4.dp).clip(RoundedCornerShape(4.dp))
                .background(outline.copy(alpha = 0.25f))
        ) {
            if (frac > 0f) {
                Box(
                    Modifier.fillMaxWidth(frac).height(4.dp).clip(RoundedCornerShape(4.dp))
                        .background(if (dimmed) muted.copy(alpha = 0.65f) else accent)
                )
            }
        }
    }
}
