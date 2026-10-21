package com.forge.app.ui.cardio

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled
import androidx.compose.ui.semantics.heading
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.theme.CardioOpenColors
import com.forge.app.ui.theme.CardioOpenType
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType

/*
 * Cardio's own pieces of the "Open panel" redesign (2026-10-21): the reading column, the date column a
 * row leads with, the outlined chips the log sheet picks with, and the three small graphs the designs
 * draw that the shared kit does not (the pace line, the strip plots, the heart-rate area). Private to
 * the package: the shared kit lives in ui/common and is not grown from here.
 */

/** The gutter every Cardio page holds to. */
internal val CARDIO_GUTTER = 24.dp

/** The air between two sections, as the Cardio designs draw it. */
internal val CARDIO_SECTION = 44.dp

internal val CardioCorner = RoundedCornerShape(4.dp)

/** "95" with " min" after it at [unitStyle] in the muted voice. */
@Composable
internal fun cardioFigure(value: String, unit: String, unitStyle: TextStyle): AnnotatedString {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    return buildAnnotatedString {
        append(value)
        if (unit.isNotEmpty()) withStyle(unitStyle.toSpanStyle().copy(color = muted)) { append(unit) }
    }
}

/** A mono change line under a figure: ink when something moved, muted otherwise. */
@Composable
internal fun CardioChange(text: String, moved: Boolean, modifier: Modifier = Modifier) {
    OpenLabel(
        text,
        modifier = modifier,
        style = OpenType.monoXS,
        color = if (moved) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant
    )
}

/**
 * One reading: mono label → serif figure → mono change line, as three share a row. The whole column
 * reads as one sentence to TalkBack through [description].
 */
@Composable
internal fun CardioReading(
    label: String,
    figure: AnnotatedString,
    change: String?,
    moved: Boolean,
    description: String,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier.clearAndSetSemantics { contentDescription = description },
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        OpenLabel(label)
        CardioFittedFigure(figure, OpenType.figureM)
        if (change != null) CardioChange(change, moved)
    }
}

/**
 * A serif figure that steps itself down (to 70% at most) rather than wrap its unit onto a second
 * line when a narrow column cannot hold it, as "12.7 mi" in a third of a 384dp phone. Past that
 * floor it wraps.
 */
@Composable
internal fun CardioFittedFigure(figure: AnnotatedString, style: TextStyle, modifier: Modifier = Modifier) {
    var shrink by remember(figure) { mutableFloatStateOf(1f) }
    val d = LocalDensity.current
    CompositionLocalProvider(LocalDensity provides Density(d.density, d.fontScale * shrink)) {
        Text(
            figure,
            style = style,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = modifier,
            onTextLayout = { if (it.lineCount > 1 && shrink > 0.71f) shrink -= 0.1f }
        )
    }
}

/** A head row: the mono label on the left and a mono reading on the right (ink when [rightInk]). */
@Composable
internal fun CardioHead(left: String, right: String? = null, rightInk: Boolean = false, modifier: Modifier = Modifier) {
    Row(
        modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        OpenLabel(left, Modifier.weight(1f))
        if (right != null) {
            OpenLabel(
                right,
                color = if (rightInk) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

/**
 * A head row whose right side is a link ("Weeks ›", "View all ›"). The label takes the room left
 * over and wraps, so a long one at a large font scale never runs into the link.
 */
@Composable
internal fun CardioLinkHead(label: String, action: String, actionLabel: String, onAction: () -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
        OpenLabel(label, Modifier.weight(1f).semantics { heading() })
        Row(
            Modifier
                .touchSlop(vertical = 16.dp, horizontal = 8.dp)
                .clickableLabeled(actionLabel, onClick = onAction)
                .padding(vertical = 16.dp, horizontal = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(2.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            OpenLabel(action)
            Icon(OpenIcons.ChevronRight, contentDescription = null, tint = muted, modifier = Modifier.size(14.dp))
        }
    }
}

/** The top row of a Cardio page: back on the left, one glyph action on the right. */
@Composable
internal fun CardioTopRow(
    onBack: (() -> Unit)?,
    actionIcon: ImageVector? = null,
    actionLabel: String = "",
    onAction: () -> Unit = {},
    trailing: (@Composable () -> Unit)? = null,
) {
    Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically) {
        if (onBack != null) OpenTopBarIcon(OpenIcons.ArrowLeft, "Back", onBack)
        Spacer(Modifier.weight(1f))
        if (actionIcon != null) OpenTopBarIcon(actionIcon, actionLabel, onAction)
        trailing?.invoke()
    }
}

/** A row's date column: a mono weekday or month over a serif day number, 34dp wide. */
@Composable
internal fun CardioDateColumn(top: String, day: String, modifier: Modifier = Modifier) {
    Column(modifier.width(34.dp)) {
        OpenLabel(top, style = OpenType.monoXS)
        Text(day, style = OpenType.figureS, color = MaterialTheme.colorScheme.onBackground)
    }
}

/**
 * An outlined pick on the log sheet (Today, a time, an activity tile, a quick pick). Selected, it is
 * washed and edged in the accent, as the design draws the chosen activity and duration.
 */
@Composable
internal fun CardioOutlineChip(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    selected: Boolean = false,
    icon: ImageVector? = null,
    height: Dp = 32.dp,
    style: TextStyle = OpenType.buttonS,
    label: String = text,
    role: Role = Role.Button,
    centered: Boolean = false,
) {
    val accent = MaterialTheme.colorScheme.primary
    val ink = MaterialTheme.colorScheme.onBackground
    Row(
        modifier
            .touchSlop(vertical = ((48.dp - height) / 2).coerceAtLeast(0.dp))
            .bounceClick(role = role, onClick = onClick)
            .padding(vertical = ((48.dp - height) / 2).coerceAtLeast(0.dp))
            .semantics { this.selected = selected; contentDescription = label }
            .heightIn(min = height)
            .clip(CardioCorner)
            .then(if (selected) Modifier.background(accent.copy(alpha = 0.15f)) else Modifier)
            .border(1.dp, if (selected) accent else CardioOpenColors.FieldEdge, CardioCorner)
            .padding(horizontal = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp, if (centered) Alignment.CenterHorizontally else Alignment.Start),
        verticalAlignment = Alignment.CenterVertically
    ) {
        if (icon != null) {
            Icon(icon, contentDescription = null, tint = if (selected) ink else MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(16.dp))
        }
        Text(text, style = style, color = ink)
    }
}

/** Delete, outlined in the error colour with its label in it (a destructive one-shot, paired with Undo). */
@Composable
internal fun CardioDangerButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val error = MaterialTheme.colorScheme.error
    Box(
        modifier
            .touchSlop(vertical = 8.dp)
            .bounceClick(role = Role.Button, onClick = onClick)
            .padding(vertical = 8.dp)
            .heightIn(min = 32.dp)
            .clip(CardioCorner)
            .border(1.dp, error.copy(alpha = 0.6f), CardioCorner)
            .padding(horizontal = 14.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(text, style = OpenType.buttonS, color = error)
    }
}

/** Edit and its like: a 32dp outlined button with an ink label. */
@Composable
internal fun CardioQuietButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    Box(
        modifier
            .touchSlop(vertical = 8.dp)
            .bounceClick(role = Role.Button, onClick = onClick)
            .padding(vertical = 8.dp)
            .heightIn(min = 32.dp)
            .clip(CardioCorner)
            .border(1.dp, CardioOpenColors.FieldEdge, CardioCorner)
            .padding(horizontal = 14.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(text, style = OpenType.buttonS, color = MaterialTheme.colorScheme.onBackground)
    }
}

/**
 * A week's Mon–Sun minutes as seven 10dp bars, 24dp tall at most. A day with nothing is a 2dp stub;
 * [lit] draws the trained days red (a week that reached the mark, or the one still running), else in
 * the quiet bar grey.
 */
@Composable
internal fun CardioMiniDayBars(perDay: List<Int>, lit: Boolean, modifier: Modifier = Modifier) {
    val accent = MaterialTheme.colorScheme.primary
    Canvas(modifier.width(94.dp).height(24.dp)) {
        val bw = 10.dp.toPx()
        val gap = 4.dp.toPx()
        val max = (perDay.maxOrNull() ?: 0).coerceAtLeast(1)
        val r = CornerRadius(1.dp.toPx())
        perDay.take(7).forEachIndexed { i, m ->
            val x = i * (bw + gap)
            val h = if (m > 0) (m.toFloat() / max * size.height).coerceAtLeast(4.dp.toPx()) else 2.dp.toPx()
            val c = when {
                m <= 0 -> CardioOpenColors.DayStub
                lit -> accent
                else -> OpenColors.Bar
            }
            drawRoundRect(c, topLeft = Offset(x, size.height - h), size = Size(bw, h), cornerRadius = r)
        }
    }
}

/**
 * A plain line over time, as the Progress tab draws pace: no gridlines and no value gutter, month
 * names under it, a ringed dot on the latest point. [points] are (x 0..1, y) with higher y drawn
 * higher; flip a pace before passing it in so up reads as faster.
 */
@Composable
internal fun CardioLineChart(
    points: List<Pair<Float, Float>>,
    xLabels: List<Pair<Float, String>>,
    description: String,
    modifier: Modifier = Modifier,
    height: Dp = 110.dp,
) {
    val accent = MaterialTheme.colorScheme.primary
    val ground = MaterialTheme.colorScheme.background
    val measurer = rememberTextMeasurer()
    Canvas(modifier.fillMaxWidth().height(height).clearAndSetSemantics { contentDescription = description }) {
        val labelRoom = 20.dp.toPx()
        val top = 8.dp.toPx()
        val bottom = size.height - labelRoom - 6.dp.toPx()
        val dot = 4.dp.toPx()
        val left = 14.dp.toPx()
        val right = size.width - dot - 2.dp.toPx()
        xLabels.forEach { (f, s) ->
            val t = measurer.measure(s, OpenType.chartLabel.copy(color = OpenColors.Glyph))
            val cx = (left + f * (right - left) - t.size.width / 2f).coerceIn(0f, size.width - t.size.width)
            drawText(t, topLeft = Offset(cx, size.height - t.size.height))
        }
        if (points.size < 2) return@Canvas
        val lo = points.minOf { it.second }
        val hi = points.maxOf { it.second }
        val span = (hi - lo).takeIf { it > 0f } ?: 1f
        val pts = points.map { (x, y) ->
            Offset(left + x * (right - left), bottom - (y - lo) / span * (bottom - top))
        }
        val path = Path().apply {
            moveTo(pts[0].x, pts[0].y)
            for (i in 1 until pts.size) {
                val p0 = pts[i - 1]
                val p1 = pts[i]
                val mx = (p0.x + p1.x) / 2f
                cubicTo(mx, p0.y, mx, p1.y, p1.x, p1.y)
            }
        }
        drawPath(path, accent, style = Stroke(2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
        drawCircle(ground, radius = dot + 1.5.dp.toPx(), center = pts.last())
        drawCircle(accent, radius = dot - 0.5.dp.toPx(), center = pts.last())
    }
}

/**
 * A strip plot: a rule across the width, a tick for every session at its value, and this one as a
 * ringed red dot. [values] and [mine] are already oriented so the right end is the better one.
 */
@Composable
internal fun CardioStripPlot(values: List<Double>, mine: Double, description: String, modifier: Modifier = Modifier) {
    val accent = MaterialTheme.colorScheme.primary
    val ground = MaterialTheme.colorScheme.background
    Canvas(modifier.fillMaxWidth().height(18.dp).clearAndSetSemantics { contentDescription = description }) {
        val cy = size.height / 2f
        drawRect(OpenColors.Track, topLeft = Offset(0f, cy - 0.5.dp.toPx()), size = Size(size.width, 1.dp.toPx()))
        val lo = values.minOrNull() ?: return@Canvas
        val hi = values.maxOrNull() ?: return@Canvas
        val pad = 8.dp.toPx()
        val span = (hi - lo).takeIf { it > 0.0 } ?: 1.0
        fun x(v: Double) = if (hi == lo) size.width / 2f else (pad + ((v - lo) / span).toFloat() * (size.width - 2 * pad))
        val tickH = 11.dp.toPx()
        val tw = 2.dp.toPx()
        values.forEach { v ->
            drawRoundRect(
                CardioOpenColors.Tick, topLeft = Offset(x(v) - tw / 2f, cy - tickH / 2f), size = Size(tw, tickH),
                cornerRadius = CornerRadius(1.dp.toPx())
            )
        }
        drawCircle(ground, radius = 6.dp.toPx(), center = Offset(x(mine), cy))
        drawCircle(accent, radius = 4.dp.toPx(), center = Offset(x(mine), cy))
    }
}

/**
 * Heart rate over a session: a red line over a red wash down to the floor, two gridlines with their
 * bpm at the left, and minutes along the bottom.
 */
@Composable
internal fun CardioAreaChart(
    points: List<Pair<Float, Float>>,
    totalMinutes: Int,
    description: String,
    modifier: Modifier = Modifier,
    height: Dp = 110.dp,
) {
    val accent = MaterialTheme.colorScheme.primary
    val measurer = rememberTextMeasurer()
    Canvas(modifier.fillMaxWidth().height(height).clearAndSetSemantics { contentDescription = description }) {
        if (points.size < 2) return@Canvas
        val left = 30.dp.toPx()
        val bottom = size.height - 22.dp.toPx()
        val top = 6.dp.toPx()
        val ys = points.map { it.second }
        val lo0 = ys.min()
        val hi0 = ys.max()
        val lo = (kotlin.math.floor(lo0 / 50f) * 50f).coerceAtMost(lo0 - 4f)
        val hi = maxOf(hi0 + 4f, lo + 20f)
        fun px(f: Float) = left + f * (size.width - left)
        fun py(v: Float) = bottom - (v - lo) / (hi - lo) * (bottom - top)
        val label = OpenType.chartLabel.copy(color = OpenColors.Glyph)
        generateSequence(kotlin.math.ceil(lo / 50f) * 50f) { it + 50f }.takeWhile { it <= hi }.forEach { g ->
            val y = py(g)
            drawLine(OpenColors.Grid, Offset(left, y), Offset(size.width, y), strokeWidth = 1.dp.toPx())
            val t = measurer.measure(g.toInt().toString(), label)
            drawText(t, topLeft = Offset(left - 6.dp.toPx() - t.size.width, y - t.size.height / 2f))
        }
        if (totalMinutes > 0) {
            val step = when {
                totalMinutes <= 20 -> 5
                totalMinutes <= 60 -> 10
                totalMinutes <= 120 -> 20
                else -> 30
            }
            var m = 0
            while (m <= totalMinutes) {
                val t = measurer.measure("$m", label)
                val cx = (px(m.toFloat() / totalMinutes) - t.size.width / 2f).coerceIn(left - t.size.width / 2f, size.width - t.size.width)
                drawText(t, topLeft = Offset(cx, bottom + 6.dp.toPx()))
                m += step
            }
        }
        val pts = points.map { Offset(px(it.first), py(it.second)) }
        val line = Path().apply { pts.forEachIndexed { i, p -> if (i == 0) moveTo(p.x, p.y) else lineTo(p.x, p.y) } }
        val area = Path().apply {
            addPath(line)
            lineTo(pts.last().x, bottom)
            lineTo(pts.first().x, bottom)
            close()
        }
        drawPath(area, accent.copy(alpha = 0.25f))
        drawPath(line, accent, style = Stroke(2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
    }
}

/** A quiet body line, for the plain sentences an empty section says. */
@Composable
internal fun CardioBodyLine(text: String, modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.onSurfaceVariant) {
    Text(text, style = OpenType.body, color = color, modifier = modifier)
}

/** The row figure on a session row: "9:31" with " /mi" smaller and muted. */
@Composable
internal fun CardioRowFigure(value: String, unit: String, modifier: Modifier = Modifier, small: Boolean = false) {
    Text(
        cardioFigure(value, unit, if (small) CardioOpenType.rowFigureUnitS else CardioOpenType.rowFigureUnit),
        style = CardioOpenType.rowFigure,
        color = MaterialTheme.colorScheme.onBackground,
        modifier = modifier
    )
}
