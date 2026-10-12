package com.tally.app.ui.kit

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.tally.app.ui.theme.Tally
import kotlin.math.max
import kotlin.math.roundToInt

/*
 * Charts in Relay's hand: hairline axes left out, ink for what was, ink-3 for what is expected,
 * colour only for state (over pace, over budget) and for category hues. Every chart carries a
 * spoken reading for TalkBack.
 */

/**
 * A week's spending as seven bars, today's in ink and the others in ink-3, days still ahead empty;
 * [daily] (a day's even share of the budget) as a dashed line; a bar over it reads amber.
 */
@Composable
fun WeekBars(values: List<Long>, today: Int, labels: List<String>, description: String, modifier: Modifier = Modifier, daily: Long? = null, height: Dp = 96.dp) {
    val c = Tally.colors
    val top = max(values.maxOrNull() ?: 0L, daily ?: 0L).coerceAtLeast(1L)
    Column(modifier.fillMaxWidth().semantics { contentDescription = description }, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Canvas(Modifier.fillMaxWidth().height(height)) {
            val n = values.size.coerceAtLeast(1)
            val slot = size.width / n
            val bar = slot * .46f
            values.forEachIndexed { i, v ->
                val x = slot * i + (slot - bar) / 2
                val h = if (v <= 0L) 0f else (size.height * v / top).coerceAtLeast(3.dp.toPx())
                val color = when {
                    i > today -> c.track
                    daily != null && v > daily -> c.waiting
                    i == today -> c.ink
                    else -> c.ink3
                }
                if (i > today || v <= 0L) {
                    drawRoundRect(c.track, Offset(x, size.height - 3.dp.toPx()), Size(bar, 3.dp.toPx()), CornerRadius(2.dp.toPx()))
                } else {
                    drawRoundRect(color, Offset(x, size.height - h), Size(bar, h), CornerRadius(4.dp.toPx()))
                }
            }
            if (daily != null && daily > 0L) {
                val y = size.height - size.height * daily / top
                drawLine(c.ink2, Offset(0f, y), Offset(size.width, y), strokeWidth = 1.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(6f, 6f)))
            }
        }
        Row(Modifier.fillMaxWidth()) {
            labels.forEachIndexed { i, l ->
                T(l, Modifier.weight(1f), Tally.type.mono, if (i == today) c.ink else c.ink3, align = TextAlign.Center)
            }
        }
    }
}

/**
 * The month as a line: what was spent by each day so far, against the even pace to the budget (a
 * straight dashed line from nothing to the budget at the month's end). Dragging along it picks a
 * day ([onScrub]); the picked day is marked.
 */
@Composable
fun SpendLine(
    cumulative: List<Long>,
    days: Int,
    description: String,
    modifier: Modifier = Modifier,
    budget: Long? = null,
    picked: Int? = null,
    onScrub: ((Int) -> Unit)? = null,
    height: Dp = 160.dp,
) {
    val c = Tally.colors
    val last = cumulative.lastOrNull() ?: 0L
    val projected = if (cumulative.isNotEmpty() && cumulative.size < days) last * days / cumulative.size else last
    val top = listOf(last, budget ?: 0L, projected).max().coerceAtLeast(1L)
    val over = budget != null && budget > 0L && last > budget
    fun pick(x: Float, width: Float) {
        if (cumulative.isEmpty() || width <= 0f || days <= 1) return
        val i = ((x / width).coerceIn(0f, 1f) * (days - 1)).roundToInt().coerceIn(0, cumulative.size - 1)
        onScrub?.invoke(i)
    }
    Canvas(
        modifier.fillMaxWidth().height(height).semantics { contentDescription = description }
            .then(
                if (onScrub == null) Modifier else Modifier
                    .pointerInput(cumulative, days) { detectTapGestures { pick(it.x, size.width.toFloat()) } }
                    .pointerInput(cumulative, days) { detectDragGestures { change, _ -> pick(change.position.x, size.width.toFloat()) } },
            ),
    ) {
        val w = size.width
        val h = size.height
        fun x(i: Int) = if (days <= 1) 0f else w * i / (days - 1)
        fun y(v: Long) = h - h * v / top
        // The floor.
        drawLine(c.track, Offset(0f, h - 1f), Offset(w, h - 1f), strokeWidth = 1.dp.toPx())
        // The even pace to the budget.
        if (budget != null && budget > 0L) {
            drawLine(c.ink3, Offset(0f, h), Offset(w, y(budget)), strokeWidth = 1.5.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(8f, 8f)))
            drawLine(c.ink3.copy(alpha = .5f), Offset(0f, y(budget)), Offset(w, y(budget)), strokeWidth = 1.dp.toPx())
        }
        if (cumulative.isEmpty()) return@Canvas
        val line = Path().apply {
            moveTo(x(0), y(cumulative[0]))
            for (i in 1 until cumulative.size) lineTo(x(i), y(cumulative[i]))
        }
        val area = Path().apply {
            addPath(line)
            lineTo(x(cumulative.size - 1), h)
            lineTo(x(0), h)
            close()
        }
        val ink = if (over) c.held else c.ink
        drawPath(area, ink.copy(alpha = .08f))
        drawPath(line, ink, style = Stroke(width = 2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
        // Where today's rate would end the month.
        if (cumulative.size < days) {
            drawLine(ink.copy(alpha = .35f), Offset(x(cumulative.size - 1), y(last)), Offset(w, y(projected)), strokeWidth = 1.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(3f, 6f)))
        }
        val p = (picked ?: (cumulative.size - 1)).coerceIn(0, cumulative.size - 1)
        drawLine(c.ink2.copy(alpha = .5f), Offset(x(p), 0f), Offset(x(p), h), strokeWidth = 1.dp.toPx())
        drawCircle(c.wall, 5.dp.toPx(), Offset(x(p), y(cumulative[p])))
        drawCircle(ink, 3.5.dp.toPx(), Offset(x(p), y(cumulative[p])))
    }
}

/** Shares of a whole as one bar of hues, largest first, with 2dp seams. */
@Composable
fun StackBar(parts: List<Pair<Color, Long>>, modifier: Modifier = Modifier, height: Dp = 10.dp) {
    val c = Tally.colors
    val total = parts.sumOf { it.second }.coerceAtLeast(1L)
    Canvas(modifier.fillMaxWidth().height(height).clip(Radii.pill)) {
        drawRect(c.track)
        var x = 0f
        val seam = 2.dp.toPx()
        parts.forEach { (color, v) ->
            if (v <= 0L) return@forEach
            val w = size.width * v / total
            drawRect(color, Offset(x, 0f), Size((w - seam).coerceAtLeast(1f), size.height))
            x += w
        }
    }
}

/**
 * Months side by side: what went out (ink) and what came in (live green, thinner) for each, the
 * running month in ink-2. [highlight] marks the month the page is reading.
 */
@Composable
fun TrendBars(spent: List<Long>, income: List<Long>, labels: List<String>, description: String, modifier: Modifier = Modifier, highlight: Int = spent.size - 1, onPick: ((Int) -> Unit)? = null, height: Dp = 130.dp) {
    val c = Tally.colors
    val top = (spent + income).maxOrNull()?.coerceAtLeast(1L) ?: 1L
    Column(modifier.fillMaxWidth().semantics { contentDescription = description }, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Canvas(
            Modifier.fillMaxWidth().height(height).then(
                if (onPick == null) Modifier else Modifier.pointerInput(spent.size) {
                    detectTapGestures { pos -> onPick(((pos.x / size.width) * spent.size).toInt().coerceIn(0, spent.size - 1)) }
                },
            ),
        ) {
            val n = spent.size.coerceAtLeast(1)
            val slot = size.width / n
            val bar = slot * .34f
            for (i in 0 until n) {
                val base = slot * i + slot * .14f
                val s = spent.getOrElse(i) { 0L }
                val inc = income.getOrElse(i) { 0L }
                val sh = size.height * s / top
                val ih = size.height * inc / top
                val on = i == highlight
                drawRoundRect(if (on) c.ink else c.ink3, Offset(base, size.height - sh), Size(bar, sh), CornerRadius(3.dp.toPx()))
                drawRoundRect(c.live.copy(alpha = if (on) 1f else .55f), Offset(base + bar + 3.dp.toPx(), size.height - ih), Size(bar * .6f, ih), CornerRadius(3.dp.toPx()))
            }
        }
        Row(Modifier.fillMaxWidth()) {
            labels.forEachIndexed { i, l -> T(l, Modifier.weight(1f), Tally.type.mono, if (i == highlight) c.ink else c.ink3, align = TextAlign.Center) }
        }
    }
}

/**
 * A month as a calendar of days, each lit by how much it took (0 unlit to 4 brightest), [lead]
 * empty cells first so the first day sits under its weekday. Days still ahead are outlined only.
 */
@Composable
fun HeatGrid(heat: List<Int>, lead: Int, elapsed: Int, weekdays: List<String>, description: String, modifier: Modifier = Modifier, picked: Int? = null, onPick: ((Int) -> Unit)? = null) {
    val c = Tally.colors
    val cells = List(lead) { -1 } + heat.indices.toList()
    Column(modifier.fillMaxWidth().semantics { contentDescription = description }, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            weekdays.forEach { T(it, Modifier.weight(1f), Tally.type.mono, c.ink3, align = TextAlign.Center) }
        }
        cells.chunked(7).forEach { week ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                for (k in 0 until 7) {
                    val day = week.getOrNull(k) ?: -1
                    val step = if (day >= 0) heat[day] else 0
                    val fill = when {
                        day < 0 -> Color.Transparent
                        day >= elapsed -> Color.Transparent
                        step == 0 -> c.slab
                        else -> c.ink.copy(alpha = .14f + .2f * step)
                    }
                    Box(
                        Modifier.weight(1f).aspectRatio(1f).clip(Radii.icon).background(fill)
                            .then(if (day >= 0 && day < elapsed && onPick != null) Modifier.clickable { onPick(day) } else Modifier)
                            .then(if (day >= 0 && day == picked) Modifier.border(1.dp, c.signal, Radii.icon) else Modifier),
                        contentAlignment = Alignment.Center,
                    ) {
                        if (day >= 0) {
                            T(
                                (day + 1).toString(),
                                style = Tally.type.mono,
                                color = when {
                                    day == picked -> c.signal
                                    day >= elapsed -> c.ink3.copy(alpha = .5f)
                                    step >= 3 -> c.wall
                                    else -> c.ink2
                                },
                            )
                        }
                    }
                }
            }
        }
    }
}

/** A small line of values, oldest first: net worth over months. */
@Composable
fun Sparkline(values: List<Long>, modifier: Modifier = Modifier, color: Color = Tally.colors.ink, height: Dp = 44.dp) {
    val c = Tally.colors
    Canvas(modifier.fillMaxWidth().height(height)) {
        if (values.size < 2) {
            drawLine(c.track, Offset(0f, size.height / 2), Offset(size.width, size.height / 2), strokeWidth = 1.dp.toPx())
            return@Canvas
        }
        val lo = values.min()
        val hi = values.max()
        val span = (hi - lo).coerceAtLeast(1L)
        fun x(i: Int) = size.width * i / (values.size - 1)
        fun y(v: Long) = size.height - 3.dp.toPx() - (size.height - 6.dp.toPx()) * (v - lo) / span
        val path = Path().apply {
            moveTo(x(0), y(values[0]))
            for (i in 1 until values.size) lineTo(x(i), y(values[i]))
        }
        drawPath(path, color, style = Stroke(width = 2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
        drawCircle(color, 3.dp.toPx(), Offset(x(values.size - 1), y(values.last())))
    }
}
