package com.margin.app.ui.common

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp

/*
 * Panels: Margin's departure from Avex's open page, at the owner's call (2026-10-04: "professional,
 * with icons, backgrounds, more details; a gym app is more discreet than this"). Avex's ground,
 * type voices and accent stay; content now sits on raised panels so a dense money screen reads as
 * organised blocks. One panel per section, never a panel inside a panel. Interactive rows inside a
 * panel are bare taps, not further boxes.
 */

val PANEL_RADIUS = 20.dp
val PANEL_PAD = 18.dp

/** A section's surface: the raised warm fill, a quiet edge, 20dp corners. */
@Composable
fun Panel(
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(PANEL_PAD),
    content: @Composable ColumnScope.() -> Unit,
) {
    val shape = RoundedCornerShape(PANEL_RADIUS)
    Column(
        modifier
            .fillMaxWidth()
            .clip(shape)
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .border(1.dp, MaterialTheme.colorScheme.outlineVariant, shape)
            .padding(contentPadding),
        content = content,
    )
}

/**
 * The screen's one lit panel: the accent washes in from the top so the hero reading sits under a
 * warm light. Used once per screen, for the figure the screen exists to show.
 */
@Composable
fun HeroPanel(
    modifier: Modifier = Modifier,
    tint: Color = MaterialTheme.colorScheme.primary,
    content: @Composable ColumnScope.() -> Unit,
) {
    val shape = RoundedCornerShape(24.dp)
    val base = MaterialTheme.colorScheme.surfaceContainer
    val brush = remember(tint, base) {
        Brush.verticalGradient(listOf(tint.copy(alpha = 0.22f), tint.copy(alpha = 0.06f), base))
    }
    Column(
        modifier
            .fillMaxWidth()
            .clip(shape)
            .background(base)
            .background(brush)
            .border(1.dp, tint.copy(alpha = 0.28f), shape)
            .padding(20.dp),
        content = content,
    )
}

/** A small rounded glyph tile: the icon that leads a panel header or a stat. */
@Composable
fun IconBadge(
    icon: ImageVector,
    modifier: Modifier = Modifier,
    tint: Color = MaterialTheme.colorScheme.primary,
    size: Dp = 32.dp,
) {
    Box(
        modifier.size(size).clip(RoundedCornerShape(10.dp)).background(tint.copy(alpha = 0.16f)),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(size * 0.56f))
    }
}

/**
 * Two ends of one line: [start] takes the room, [end] sits at the far edge. When the longest word
 * of [start] and the whole of [end] cannot share the width (a 200% font, a long reading), [end]
 * drops under [start] instead of squeezing it until a word breaks in the middle.
 */
@Composable
fun EndsRow(
    start: @Composable () -> Unit,
    end: (@Composable () -> Unit)?,
    modifier: Modifier = Modifier,
    gap: Dp = 12.dp,
    stackedGap: Dp = 2.dp,
) {
    if (end == null) {
        Box(modifier) { start() }
        return
    }
    Layout(contents = listOf(start, end), modifier = modifier) { (starts, ends), constraints ->
        val first = starts.first()
        val last = ends.first()
        val loose = constraints.copy(minWidth = 0, minHeight = 0)
        val gapPx = gap.roundToPx()
        val endPlaceable = last.measure(loose)
        val bounded = constraints.hasBoundedWidth
        val room = constraints.maxWidth
        val sideBySide = !bounded || first.minIntrinsicWidth(Constraints.Infinity) + gapPx + endPlaceable.width <= room
        if (sideBySide) {
            val startMax = if (bounded) room - gapPx - endPlaceable.width else Constraints.Infinity
            val startPlaceable = first.measure(loose.copy(maxWidth = startMax.coerceAtLeast(0)))
            val width = if (bounded) room else startPlaceable.width + gapPx + endPlaceable.width
            val height = maxOf(startPlaceable.height, endPlaceable.height).coerceAtLeast(constraints.minHeight)
            layout(width, height) {
                startPlaceable.placeRelative(0, (height - startPlaceable.height) / 2)
                endPlaceable.placeRelative(width - endPlaceable.width, (height - endPlaceable.height) / 2)
            }
        } else {
            val startPlaceable = first.measure(loose)
            val below = stackedGap.roundToPx()
            val height = (startPlaceable.height + below + endPlaceable.height).coerceAtLeast(constraints.minHeight)
            layout(room, height) {
                startPlaceable.placeRelative(0, 0)
                endPlaceable.placeRelative(0, startPlaceable.height + below)
            }
        }
    }
}

/**
 * A panel's header: icon tile, the mono label, and either a reading or a `view all →` at the end.
 * The label names the panel; it is not a kicker above a heading.
 */
@Composable
fun PanelHeader(
    icon: ImageVector,
    title: String,
    modifier: Modifier = Modifier,
    meta: String? = null,
    action: String? = null,
    onAction: (() -> Unit)? = null,
    tint: Color = MaterialTheme.colorScheme.primary,
) {
    val actionLabel = action?.takeIf { onAction != null }
    val reading = meta?.takeIf { it.isNotBlank() }
    Row(
        modifier.fillMaxWidth().heightIn(min = 40.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        IconBadge(icon, tint = tint)
        EndsRow(
            start = {
                Text(
                    title.uppercase(),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.semantics { heading() },
                )
            },
            end = if (actionLabel != null || reading != null) {
                {
                    if (actionLabel != null && onAction != null) {
                        TextAction(actionLabel, onAction, color = MaterialTheme.colorScheme.primary)
                    } else if (reading != null) {
                        Text(reading, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            } else {
                null
            },
            modifier = Modifier.weight(1f),
        )
    }
}

/**
 * One reading as a tile: glyph, mono label, the value in serif, an optional detail line. For a
 * grid of DIFFERENT readings (per day, vs last month, biggest category), never a wall of the same.
 */
@Composable
fun StatTile(
    icon: ImageVector,
    label: String,
    value: String,
    modifier: Modifier = Modifier,
    detail: String? = null,
    tint: Color = MaterialTheme.colorScheme.primary,
    valueColor: Color = MaterialTheme.colorScheme.onBackground,
) {
    val shape = RoundedCornerShape(18.dp)
    Column(
        modifier
            .clip(shape)
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .border(1.dp, MaterialTheme.colorScheme.outlineVariant, shape)
            .semantics(mergeDescendants = true) { contentDescription = listOfNotNull(label, value, detail).joinToString(", ") }
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        IconBadge(icon, tint = tint, size = 30.dp)
        Column(Modifier.clearAndSetSemantics { }, verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label.uppercase(), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(value, style = MaterialTheme.typography.headlineSmall, color = valueColor)
            if (detail != null) {
                Text(detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/** A small pill of context inside a panel: glyph and a few words ("$58 a day", "17 days left"). */
@Composable
fun StatChip(icon: ImageVector, text: String, modifier: Modifier = Modifier) {
    Row(
        modifier
            .clip(RoundedCornerShape(50))
            .background(MaterialTheme.colorScheme.surfaceContainerHighest.copy(alpha = 0.7f))
            .padding(horizontal = 12.dp, vertical = 7.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(16.dp))
        Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onBackground)
    }
}

/**
 * Spend per day as rounded bars with the daily allowance as a dashed line: the week (or any short
 * run of days) at a glance. [highlight] is today's index, drawn in the accent; past days take the
 * secondary rung; days still ahead draw as empty stubs. Bars over the allowance take the error
 * colour, a true state.
 */
@Composable
fun DayBars(
    values: List<Long>,
    labels: List<String>,
    highlight: Int,
    description: String,
    modifier: Modifier = Modifier,
    allowance: Long? = null,
    height: Dp = 96.dp,
) {
    val accent = MaterialTheme.colorScheme.primary
    val past = MaterialTheme.colorScheme.primary.copy(alpha = 0.55f)
    val over = MaterialTheme.colorScheme.error
    val track = MaterialTheme.colorScheme.surfaceContainerHighest
    val guide = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f)
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    val progress = rememberDrawIn(1f)
    Column(modifier.fillMaxWidth()) {
        Canvas(Modifier.fillMaxWidth().height(height).semantics { contentDescription = description }) {
            if (values.isEmpty()) return@Canvas
            val n = values.size
            val gap = 8.dp.toPx()
            val barW = (size.width - gap * (n - 1)) / n
            val max = maxOf(values.maxOrNull() ?: 0L, allowance ?: 0L, 1L).toFloat()
            val r = CornerRadius(minOf(barW / 2f, 6.dp.toPx()))
            values.forEachIndexed { i, v ->
                val slot = if (rtl) n - 1 - i else i
                val x = slot * (barW + gap)
                drawRoundRect(track, topLeft = Offset(x, 0f), size = Size(barW, size.height), cornerRadius = r)
                if (i > highlight) return@forEachIndexed
                val h = size.height * (v / max) * progress.value
                if (h <= 0f) return@forEachIndexed
                val color = when {
                    allowance != null && allowance > 0 && v > allowance -> over
                    i == highlight -> accent
                    else -> past
                }
                drawRoundRect(color, topLeft = Offset(x, size.height - h), size = Size(barW, h), cornerRadius = r)
            }
            if (allowance != null && allowance > 0) {
                val y = size.height * (1f - allowance / max)
                drawLine(
                    guide, Offset(0f, y), Offset(size.width, y), strokeWidth = 1.5.dp.toPx(),
                    pathEffect = PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 5.dp.toPx())),
                )
            }
        }
        Spacer(Modifier.height(8.dp))
        Row(Modifier.fillMaxWidth().clearAndSetSemantics { }) {
            labels.forEachIndexed { i, label ->
                Text(
                    label,
                    modifier = Modifier.weight(1f),
                    style = MaterialTheme.typography.labelMedium,
                    color = if (i == highlight) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                )
            }
        }
    }
}

/** A coloured dot and a label: a chart legend entry. */
@Composable
fun LegendDot(color: Color, label: String, modifier: Modifier = Modifier) {
    Row(modifier, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Box(Modifier.size(8.dp).clip(RoundedCornerShape(50)).background(color))
        Text(label, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** Vertical space between panels on a page. */
val PANEL_GAP = 14.dp

/** Space for a width-only spacer in rows of tiles. */
@Composable
fun TileGap() = Spacer(Modifier.width(PANEL_GAP))
