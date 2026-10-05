package com.tally.app.ui.common

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
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
import com.tally.app.ui.theme.MonoSectionAnchor

/*
 * Open editorial, Avex's language (the owner's call, 2026-10-05: "I don't like these big boxes,
 * look at what I did for Avex"). Content sits straight on the page: no fill, no edge around
 * anything that cannot be tapped. A section is its mono header and the air around it; a reading is
 * a serif figure over a mono label; a line of context is a quiet muted line. Surfaces are earned by
 * interactivity: slab groups, chips, fields and buttons keep theirs.
 */

/**
 * An open section: its content straight on the page. No fill and no edge, because a box is a
 * promise of a tap; the air around it and its [PanelHeader] are the only separator.
 */
@Composable
fun Panel(
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(0.dp),
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(modifier.fillMaxWidth().padding(contentPadding), content = content)
}

/**
 * The screen's lead section: the figure the screen exists to show, open on the page like every
 * other section. It is told apart by its serif figure, never by a box or a wash. Used once per
 * screen.
 */
@Composable
fun HeroPanel(
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(modifier.fillMaxWidth(), content = content)
}

/**
 * Two ends of one line: [start] takes the room, [end] sits at the far edge. When the longest word
 * of [start] and the whole of [end] cannot share the width (a 200% font, a long reading), [end]
 * drops under [start] instead of squeezing it until a word breaks in the middle.
 *
 * With [keepStartOnOneLine] the bar is higher: [end] drops as soon as [start] would wrap at all,
 * so a short label ("IN AND OUT") never stacks word by word beside its reading. Panel headers use
 * it; rows and group headers, whose start is a sentence that may wrap, keep the longest-word rule.
 */
@Composable
fun EndsRow(
    start: @Composable () -> Unit,
    end: (@Composable () -> Unit)?,
    modifier: Modifier = Modifier,
    gap: Dp = 12.dp,
    stackedGap: Dp = 2.dp,
    keepStartOnOneLine: Boolean = false,
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
        val startNeeds = if (keepStartOnOneLine) {
            first.maxIntrinsicWidth(Constraints.Infinity)
        } else {
            first.minIntrinsicWidth(Constraints.Infinity)
        }
        val sideBySide = !bounded || endsShareLine(startNeeds, gapPx, endPlaceable.width, room)
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
 * The [EndsRow] decision: the start's needed width (its longest word, or its whole line when it
 * must not wrap), the gap and the end fit the room, so the two share one line.
 */
internal fun endsShareLine(startNeeds: Int, gap: Int, endWidth: Int, room: Int): Boolean =
    startNeeds + gap + endWidth <= room

/**
 * A section's mono anchor, Avex's: the label in small caps, and either a reading or a `view all →`
 * at the end. No glyph and no rule: the header and the air above it are the separator. The label
 * keeps one line: when it and the end cannot share the width whole, the end drops under it. A
 * [tint] other than the accent colours the label for a true state (over budget, money in).
 */
@Composable
fun PanelHeader(
    title: String,
    modifier: Modifier = Modifier,
    meta: String? = null,
    action: String? = null,
    onAction: (() -> Unit)? = null,
    tint: Color = MaterialTheme.colorScheme.primary,
) {
    val actionLabel = action?.takeIf { onAction != null }
    val reading = meta?.takeIf { it.isNotBlank() }
    val labelColor = if (tint == MaterialTheme.colorScheme.primary) MaterialTheme.colorScheme.onSurfaceVariant else tint
    EndsRow(
        start = {
            Text(
                title.uppercase(),
                style = MonoSectionAnchor,
                color = labelColor,
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
        modifier = modifier.fillMaxWidth().heightIn(min = 32.dp),
        keepStartOnOneLine = true,
    )
}

/**
 * One open reading: the value in serif straight on the page, its mono label under it, an optional
 * detail line. Avex's figure, with room for the one line that explains it. For a row of DIFFERENT
 * readings, never a wall of the same. TalkBack hears label, value, detail.
 */
@Composable
fun StatTile(
    label: String,
    value: String,
    modifier: Modifier = Modifier,
    detail: String? = null,
    valueColor: Color = MaterialTheme.colorScheme.onBackground,
) {
    Column(
        modifier.semantics(mergeDescendants = true) { contentDescription = listOfNotNull(label, value, detail).joinToString(", ") },
    ) {
        Column(Modifier.clearAndSetSemantics { }) {
            Text(value, style = MaterialTheme.typography.headlineSmall, color = valueColor)
            Spacer(Modifier.height(2.dp))
            Text(label.uppercase(), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            if (detail != null) {
                Spacer(Modifier.height(6.dp))
                Text(detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/** A quiet line of context under a figure: a muted glyph and a few words ("$58 a day for 17 days"). No pill. */
@Composable
fun StatChip(icon: ImageVector, text: String, modifier: Modifier = Modifier) {
    Row(
        modifier.padding(end = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(16.dp))
        Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/**
 * Spend per day as rounded bars with the daily allowance as a dashed line: the week (or any short
 * run of days) at a glance. [highlight] is today's index, drawn in the accent; past days take the
 * secondary rung; days still ahead draw as empty stubs. Bars over the allowance take the error
 * colour, a true state.
 *
 * The first [beforeCount] days fall before the period began (a week can open in last month):
 * they draw in the muted rung, initials dimmed, and are never held to this period's allowance.
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
    beforeCount: Int = 0,
) {
    val accent = MaterialTheme.colorScheme.primary
    val past = MaterialTheme.colorScheme.primary.copy(alpha = 0.55f)
    val before = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f)
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
                    i < beforeCount -> before
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
                    color = when {
                        i < beforeCount -> MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.65f)
                        i == highlight -> MaterialTheme.colorScheme.onBackground
                        else -> MaterialTheme.colorScheme.onSurfaceVariant
                    },
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

/** Air between two open sections on a page: with the section's header, the only separator. */
val PANEL_GAP = 28.dp

/** Space between two figures side by side. */
val FIGURE_GAP = 20.dp

/** Space for a width-only spacer in rows of figures. */
@Composable
fun TileGap() = Spacer(Modifier.width(FIGURE_GAP))
