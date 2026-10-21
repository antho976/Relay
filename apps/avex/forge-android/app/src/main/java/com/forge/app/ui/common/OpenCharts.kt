package com.forge.app.ui.common

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.layout
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenType
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.log10
import kotlin.math.pow
import kotlin.math.roundToInt

/*
 * The redesign's graphs (2026-10-21): a trend line against the span before it, columns, and a bar
 * list. Drawn as the design canvas draws its chart nodes: two faint gridlines with their values at
 * the left, month names under the plot, 10sp quiet labels, red only for the line or bars that are
 * the point. Every graph says what it shows to TalkBack through [description].
 */

/** One point on a trend graph: [x] from 0 (the range's start) to 1 (now). */
data class ChartPoint(val x: Float, val y: Float)

private val Y_GUTTER = 30.dp
private val X_GUTTER = 22.dp

/** Two round values inside [lo, hi] for the gridlines, e.g. 200 and 220. */
internal fun gridValues(lo: Float, hi: Float): List<Float> {
    val span = (hi - lo).takeIf { it > 0f } ?: return listOf(lo)
    val raw = span / 2f
    val mag = 10f.pow(floor(log10(raw)))
    val step = listOf(1f, 2f, 2.5f, 5f, 10f).map { it * mag }.first { it >= raw * 0.6f }
    val first = ceil(lo / step) * step
    return generateSequence(first) { it + step }.takeWhile { it <= hi }.take(3).toList()
}

private fun label(v: Float): String = when {
    abs(v) >= 10_000 -> "${(v / 1000).roundToInt()}k"
    abs(v) >= 1000 -> "%.1fk".format(v / 1000)
    v == v.roundToInt().toFloat() -> v.roundToInt().toString()
    else -> "%.1f".format(v)
}

/**
 * A trend over a range: [current] in red, [previous] (the same span before, lined up under it) in a
 * faint line, deload weeks as [bands], PRs as rings on the line at [prIndices], coach changes as
 * diamonds along the bottom at [coachMarks]. With [readout], touching the graph shows a guide, a dot
 * on the nearest point and a small card with what [readout] says about it.
 */
@Composable
fun OpenTrendChart(
    current: List<ChartPoint>,
    previous: List<ChartPoint>,
    xLabels: List<Pair<Float, String>>,
    modifier: Modifier = Modifier,
    height: Dp = 190.dp,
    prIndices: List<Int> = emptyList(),
    coachMarks: List<Float> = emptyList(),
    bands: List<Pair<Float, Float>> = emptyList(),
    smooth: Boolean = true,
    currentColor: Color = MaterialTheme.colorScheme.primary,
    previousColor: Color = OpenColors.Previous,
    readout: ((Int) -> AnnotatedString)? = null,
    description: String,
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val ground = MaterialTheme.colorScheme.background
    val measurer = rememberTextMeasurer()
    var scrub by remember(current) { mutableStateOf<Int?>(null) }
    val all = (current + previous).map { it.y }
    val lo0 = all.minOrNull() ?: 0f
    val hi0 = all.maxOrNull() ?: 1f
    val pad = ((hi0 - lo0) * 0.18f).coerceAtLeast(if (hi0 == lo0) maxOf(1f, abs(hi0) * 0.05f) else 0f)
    val lo = lo0 - pad
    val hi = hi0 + pad
    val grid = remember(lo, hi) { gridValues(lo, hi) }

    BoxWithConstraints(modifier.fillMaxWidth().height(height)) {
        val widthPx = constraints.maxWidth.toFloat()
        Canvas(
            Modifier
                .fillMaxWidth()
                .height(height)
                .clearAndSetSemantics { contentDescription = description }
                .then(
                    if (readout == null || current.isEmpty()) Modifier
                    else Modifier.pointerInput(current) {
                        val left = Y_GUTTER.toPx()
                        fun nearest(x: Float): Int {
                            val f = ((x - left) / (size.width - left)).coerceIn(0f, 1f)
                            return current.indices.minBy { abs(current[it].x - f) }
                        }
                        // Sideways only: a vertical swipe that starts on the graph still scrolls the page.
                        detectHorizontalDragGestures(
                            onDragStart = { scrub = nearest(it.x) },
                            onDragEnd = { scrub = null },
                            onDragCancel = { scrub = null },
                        ) { change, _ -> scrub = nearest(change.position.x); change.consume() }
                    }.pointerInput(current) {
                        val left = Y_GUTTER.toPx()
                        detectTapGestures(onPress = { pos ->
                            val f = ((pos.x - left) / (size.width - left)).coerceIn(0f, 1f)
                            scrub = current.indices.minBy { abs(current[it].x - f) }
                            tryAwaitRelease()
                            scrub = null
                        })
                    }
                )
        ) {
            val left = Y_GUTTER.toPx()
            val bottom = size.height - X_GUTTER.toPx()
            val top = 6.dp.toPx()
            val w = size.width - left
            fun px(x: Float) = left + x * w
            fun py(y: Float) = bottom - (y - lo) / (hi - lo) * (bottom - top)

            bands.forEach { (a, b) ->
                drawRoundRect(
                    OpenColors.Band, topLeft = Offset(px(a), top), size = Size((px(b) - px(a)).coerceAtLeast(6.dp.toPx()), bottom - top),
                    cornerRadius = CornerRadius(2.dp.toPx())
                )
            }
            grid.forEach { g ->
                val y = py(g)
                drawLine(OpenColors.Grid, Offset(left, y), Offset(size.width, y), strokeWidth = 1.dp.toPx())
                val t = measurer.measure(label(g), OpenType.chartLabel.copy(color = OpenColors.Glyph))
                drawText(t, topLeft = Offset(left - 6.dp.toPx() - t.size.width, y - t.size.height / 2f))
            }
            xLabels.forEach { (x, s) ->
                if (s.isEmpty()) return@forEach
                val t = measurer.measure(s, OpenType.chartLabel.copy(color = OpenColors.Glyph))
                val cx = (px(x) - t.size.width / 2f).coerceIn(left, size.width - t.size.width)
                drawText(t, topLeft = Offset(cx, bottom + 6.dp.toPx()))
            }
            drawSeries(previous.map { Offset(px(it.x), py(it.y)) }, previousColor, 2.dp.toPx(), smooth)
            drawSeries(current.map { Offset(px(it.x), py(it.y)) }, currentColor, 2.dp.toPx(), smooth)
            prIndices.mapNotNull { current.getOrNull(it) }.forEach { p ->
                val c = Offset(px(p.x), py(p.y))
                drawCircle(ground, 4.dp.toPx(), c)
                drawCircle(ink, 3.25.dp.toPx(), c, style = Stroke(1.5.dp.toPx()))
            }
            coachMarks.forEach { x ->
                val c = Offset(px(x), bottom - 8.dp.toPx())
                val r = 3.2.dp.toPx()
                rotate(45f, c) { drawRect(OpenColors.Glyph, topLeft = Offset(c.x - r, c.y - r), size = Size(2 * r, 2 * r)) }
            }
            scrub?.let { i ->
                val p = current[i]
                val x = px(p.x)
                drawLine(OpenColors.Guide, Offset(x, top + 22.dp.toPx()), Offset(x, bottom), strokeWidth = 1.dp.toPx())
                previous.minByOrNull { abs(it.x - p.x) }?.let { q ->
                    drawCircle(OpenColors.Rule, 4.dp.toPx(), Offset(x, py(q.y)))
                }
                drawCircle(ground, 7.5.dp.toPx(), Offset(x, py(p.y)))
                drawCircle(currentColor, 4.5.dp.toPx(), Offset(x, py(p.y)))
            }
        }
        val s = scrub
        if (s != null && readout != null) {
            val text = readout(s)
            val leftPx = with(androidx.compose.ui.platform.LocalDensity.current) { Y_GUTTER.toPx() }
            val xPx = leftPx + current[s].x * (widthPx - leftPx)
            Box(
                Modifier
                    .layout { m, c ->
                        val pl = m.measure(c.copy(minWidth = 0, minHeight = 0))
                        layout(c.maxWidth, pl.height) {
                            val x = (xPx - pl.width / 2f).roundToInt().coerceIn(0, (c.maxWidth - pl.width).coerceAtLeast(0))
                            pl.place(x, 0)
                        }
                    }
            ) {
                Box(
                    Modifier
                        .background(MaterialTheme.colorScheme.surfaceContainerHigh, RoundedCornerShape(4.dp))
                        .border(1.dp, OpenColors.Edge, RoundedCornerShape(4.dp))
                        .padding(horizontal = 8.dp, vertical = 5.dp)
                ) {
                    Text(text, style = OpenType.monoXS.copy(letterSpacing = OpenType.chartLabel.letterSpacing), color = ink)
                }
            }
        }
    }
}

private fun DrawScope.drawSeries(pts: List<Offset>, color: Color, width: Float, smooth: Boolean) {
    if (pts.size < 2) {
        pts.firstOrNull()?.let { drawCircle(color, width * 1.5f, it) }
        return
    }
    val path = Path().apply {
        moveTo(pts[0].x, pts[0].y)
        if (!smooth || pts.size < 3) {
            pts.drop(1).forEach { lineTo(it.x, it.y) }
        } else {
            // Monotone cubic (Fritsch-Carlson): passes through every point, so a PR ring sits on the
            // line, and never overshoots between two of them, so a curve never shows a value the
            // lift did not reach.
            val n = pts.size
            val d = FloatArray(n - 1) { i -> (pts[i + 1].y - pts[i].y) / (pts[i + 1].x - pts[i].x).coerceAtLeast(1e-3f) }
            val m = FloatArray(n)
            m[0] = d[0]; m[n - 1] = d[n - 2]
            for (i in 1 until n - 1) m[i] = if (d[i - 1] * d[i] <= 0f) 0f else (d[i - 1] + d[i]) / 2f
            for (i in 0 until n - 1) {
                if (d[i] == 0f) { m[i] = 0f; m[i + 1] = 0f; continue }
                val a = m[i] / d[i]; val b = m[i + 1] / d[i]
                val h = a * a + b * b
                if (h > 9f) { val t = 3f / kotlin.math.sqrt(h); m[i] = t * a * d[i]; m[i + 1] = t * b * d[i] }
            }
            for (i in 0 until n - 1) {
                val p0 = pts[i]; val p1 = pts[i + 1]
                val dx = (p1.x - p0.x) / 3f
                cubicTo(p0.x + dx, p0.y + m[i] * dx, p1.x - dx, p1.y - m[i + 1] * dx, p1.x, p1.y)
            }
        }
    }
    drawPath(path, color, style = Stroke(width, cap = StrokeCap.Round, join = StrokeJoin.Round))
}

/**
 * Columns along a baseline with [colors] per column (red for the one that matters), optional
 * [valueLabels] over each column and [columnLabels] under each, or month names at [xLabels].
 */
@Composable
fun OpenColumnChart(
    values: List<Float>,
    colors: List<Color>,
    modifier: Modifier = Modifier,
    height: Dp = 130.dp,
    xLabels: List<Pair<Float, String>> = emptyList(),
    columnLabels: List<String>? = null,
    valueLabels: Boolean = false,
    gapFraction: Float = 0.3f,
    description: String,
) {
    val measurer = rememberTextMeasurer()
    val labelColor = MaterialTheme.colorScheme.onSurfaceVariant
    Canvas(modifier.fillMaxWidth().height(height).clearAndSetSemantics { contentDescription = description }) {
        if (values.isEmpty()) return@Canvas
        val labelRoom = if (columnLabels != null || xLabels.isNotEmpty()) 18.dp.toPx() else 0f
        val topRoom = if (valueLabels) 16.dp.toPx() else 0f
        val bottom = size.height - labelRoom
        val n = values.size
        val slot = size.width / n
        val bw = slot * (1f - gapFraction)
        val max = values.max().takeIf { it > 0f } ?: 1f
        values.forEachIndexed { i, v ->
            val h = (v / max * (bottom - topRoom)).coerceAtLeast(if (v > 0f) 2.dp.toPx() else 0f)
            val x = i * slot + (slot - bw) / 2f
            drawRoundRect(colors.getOrElse(i) { OpenColors.Bar }, topLeft = Offset(x, bottom - h), size = Size(bw, h), cornerRadius = CornerRadius(2.dp.toPx()))
            if (valueLabels && v > 0f) {
                val t = measurer.measure(label(v), OpenType.chartLabel.copy(color = labelColor))
                drawText(t, topLeft = Offset(x + bw / 2f - t.size.width / 2f, bottom - h - t.size.height - 3.dp.toPx()))
            }
            columnLabels?.getOrNull(i)?.let { s ->
                val t = measurer.measure(s, OpenType.chartLabel.copy(color = labelColor))
                drawText(t, topLeft = Offset(x + bw / 2f - t.size.width / 2f, bottom + 5.dp.toPx()))
            }
        }
        xLabels.forEach { (f, s) ->
            if (s.isEmpty()) return@forEach
            val t = measurer.measure(s, OpenType.chartLabel.copy(color = OpenColors.Glyph))
            val cx = (f * size.width - t.size.width / 2f).coerceIn(0f, size.width - t.size.width)
            drawText(t, topLeft = Offset(cx, bottom + 5.dp.toPx()))
        }
    }
}

/**
 * A list of named bars, longest first as given: the name right-aligned in a fixed column, the bar,
 * and its value after it. [lit] rows draw red (a muscle under its range); the rest stay quiet.
 */
@Composable
fun OpenBarList(
    rows: List<Pair<String, Float>>,
    lit: Set<Int>,
    modifier: Modifier = Modifier,
    labelWidth: Dp = 76.dp,
    description: String,
) {
    val accent = MaterialTheme.colorScheme.primary
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val max = rows.maxOfOrNull { it.second }?.takeIf { it > 0f } ?: 1f
    Column(
        modifier.fillMaxWidth().clearAndSetSemantics { contentDescription = description },
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        rows.forEachIndexed { i, (name, v) ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    name, style = OpenType.chartRowLabel, color = muted, textAlign = TextAlign.End,
                    modifier = Modifier.width(labelWidth)
                )
                Box(Modifier.width(8.dp))
                BoxWithConstraints(Modifier.weight(1f)) {
                    val barMax = maxWidth - 28.dp
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Box(
                            Modifier
                                .width(barMax * (v / max))
                                .height(11.dp)
                                .background(if (i in lit) accent else OpenColors.BarQuiet, RoundedCornerShape(1.dp))
                        )
                        Box(Modifier.width(6.dp))
                        Text(label(v), style = OpenType.chartRowLabel, color = muted)
                    }
                }
            }
        }
    }
}

/** A graph's key: short line, ring, diamond and band swatches, each with its word. */
@Composable
fun OpenChartKey(items: List<Pair<ChartKeyMark, String>>, modifier: Modifier = Modifier) {
    val ink = MaterialTheme.colorScheme.onBackground
    val ground = MaterialTheme.colorScheme.background
    androidx.compose.foundation.layout.FlowRow(
        modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        items.forEach { (mark, word) ->
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                Canvas(Modifier.size(12.dp)) {
                    val c = Offset(size.width / 2f, size.height / 2f)
                    when (mark) {
                        ChartKeyMark.PREVIOUS -> drawRect(OpenColors.Rule, topLeft = Offset(0f, c.y - 1.dp.toPx()), size = Size(size.width, 2.dp.toPx()))
                        ChartKeyMark.PR -> { drawCircle(ground, 4.dp.toPx(), c); drawCircle(ink, 3.25.dp.toPx(), c, style = Stroke(1.5.dp.toPx())) }
                        ChartKeyMark.COACH -> rotate(45f, c) { val r = 3.2.dp.toPx(); drawRect(OpenColors.Glyph, Offset(c.x - r, c.y - r), Size(2 * r, 2 * r)) }
                        ChartKeyMark.DELOAD -> drawRoundRect(OpenColors.BandKey, topLeft = Offset(1.dp.toPx(), 1.dp.toPx()), size = Size(10.dp.toPx(), 10.dp.toPx()), cornerRadius = CornerRadius(2.dp.toPx()))
                    }
                }
                OpenLabel(word, style = OpenType.monoXS)
            }
        }
    }
}

enum class ChartKeyMark { PREVIOUS, PR, COACH, DELOAD }
