package com.tally.app.ui.common

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import com.tally.app.ui.theme.TallyMotion

/**
 * A value that draws in ONCE: the first time its owner composes, from zero. Saved, so a row that
 * scrolls away and back (LazyColumn keeps saveable state) shows its settled value instead of
 * replaying the reveal. Read [Animatable.value] in a draw lambda, never in composition.
 */
@Composable
fun rememberDrawIn(target: Float): Animatable<Float, *> {
    var drawn by rememberSaveable { mutableStateOf(false) }
    val anim = remember { Animatable(if (drawn) target else 0f) }
    LaunchedEffect(target) {
        anim.animateTo(target, TallyMotion.draw())
        drawn = true
    }
    return anim
}

/**
 * THE signature mark: a budget meter with its pace tick.
 *
 * The track is the budget; the fill is what is spent; the thin tick stands where an even spend
 * would be today. Fill past the tick is the reading. Past the budget the scale stretches to the
 * spend, the budget becomes a notch, and the overrun draws in the error colour, so "over" is a
 * visible length rather than a full bar that hides how far.
 *
 * [paceFraction] null draws no tick (a goal with no date, a past month).
 */
@Composable
fun PaceMeter(
    spentFraction: Float,
    paceFraction: Float?,
    description: String,
    modifier: Modifier = Modifier,
    height: Dp = 10.dp,
    fill: Color = MaterialTheme.colorScheme.primary,
) {
    val track = MaterialTheme.colorScheme.surfaceContainerHighest
    val over = MaterialTheme.colorScheme.error
    val tick = MaterialTheme.colorScheme.onBackground
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    val progress = rememberDrawIn(spentFraction.coerceAtLeast(0f))
    Canvas(
        modifier
            .fillMaxWidth()
            .height(height + 8.dp)
            .semantics { contentDescription = description }
    ) {
        val barH = height.toPx()
        val top = (size.height - barH) / 2f
        val r = CornerRadius(barH / 2f)
        val value = progress.value
        val scaleMax = maxOf(1f, spentFraction)
        val budgetX = size.width / scaleMax
        val fillX = size.width * (value / scaleMax).coerceIn(0f, 1f)

        fun x(px: Float) = if (rtl) size.width - px else px
        fun bar(from: Float, to: Float, color: Color) {
            if (to <= from) return
            val l = minOf(x(from), x(to))
            drawRoundRect(color, topLeft = Offset(l, top), size = Size(to - from, barH), cornerRadius = r)
        }

        bar(0f, size.width, track)
        bar(0f, minOf(fillX, budgetX), fill)
        if (fillX > budgetX) {
            bar(budgetX, fillX, over)
            // The budget's own edge, so the overrun reads as "this much past".
            drawTick(x(budgetX), top - 2.dp.toPx(), barH + 4.dp.toPx(), track, 2.dp.toPx())
        }
        if (paceFraction != null) {
            drawTick(x(size.width * (paceFraction / scaleMax).coerceIn(0f, 1f)), 0f, size.height, tick, 2.dp.toPx())
        }
    }
}

private fun DrawScope.drawTick(x: Float, top: Float, h: Float, color: Color, w: Float) {
    drawRoundRect(color, topLeft = Offset(x - w / 2f, top), size = Size(w, h), cornerRadius = CornerRadius(w / 2f))
}

/** A thin ranked-comparison bar (3dp), the Stats-per-lift look from Avex. */
@Composable
fun ThinBar(fraction: Float, color: Color, description: String, modifier: Modifier = Modifier) {
    val track = MaterialTheme.colorScheme.surfaceContainerHigh
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    val progress = rememberDrawIn(fraction.coerceIn(0f, 1f))
    Canvas(modifier.fillMaxWidth().height(4.dp).semantics { contentDescription = description }) {
        val r = CornerRadius(size.height / 2f)
        drawRoundRect(track, cornerRadius = r)
        val w = size.width * progress.value
        if (w > 0f) drawRoundRect(color, topLeft = Offset(if (rtl) size.width - w else 0f, 0f), size = Size(w, size.height), cornerRadius = r)
    }
}

/**
 * Shares of a whole as one bar of coloured segments with 2dp gaps: where the month went, read by
 * colour. Segments under 1% still get a sliver so a category never vanishes from its own total.
 */
@Composable
fun StackedBar(segments: List<Pair<Float, Color>>, description: String, modifier: Modifier = Modifier, height: Dp = 14.dp) {
    val track = MaterialTheme.colorScheme.surfaceContainerHigh
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    val progress = rememberDrawIn(1f)
    Canvas(modifier.fillMaxWidth().height(height).semantics { contentDescription = description }) {
        val r = CornerRadius(4.dp.toPx())
        if (segments.isEmpty()) {
            drawRoundRect(track, cornerRadius = r)
            return@Canvas
        }
        val gap = 2.dp.toPx()
        val total = segments.sumOf { it.first.toDouble() }.toFloat().takeIf { it > 0f } ?: 1f
        val usable = size.width - gap * (segments.size - 1)
        val reveal = size.width * progress.value
        var cursor = 0f
        segments.forEach { (share, color) ->
            val w = maxOf(usable * share / total, 2.dp.toPx())
            val end = minOf(cursor + w, reveal)
            if (end > cursor) {
                val l = if (rtl) size.width - end else cursor
                drawRoundRect(color, topLeft = Offset(l, 0f), size = Size(end - cursor, size.height), cornerRadius = r)
            }
            cursor += w + gap
        }
    }
}
