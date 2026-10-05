package com.tally.app.ui.insights

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
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
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.bounceClick
import com.tally.app.ui.common.rememberDrawIn
import com.tally.core.BudgetPeriod
import java.time.DayOfWeek
import java.time.LocalDate

/*
 * The three marks Insights draws on its own: the month line with its pace diagonal, the six-month
 * bars, and the heat grid. All open on the panel (no plot frame), mono axis labels, drawn in once.
 */

/** Alpha of the accent for each heat step; 0 is the unlit cell. Data steps, not decoration. */
private val HEAT_ALPHA = floatArrayOf(0f, 0.25f, 0.45f, 0.7f, 1f)

/**
 * Cumulative spend by day across the period against the budget's even pace. The line stops at
 * today; days still ahead are not drawn. Tap or drag across it to read any day: [selected] is the
 * scrubbed day (null draws no marker) and [onSelect] receives the day under the finger.
 */
@Composable
internal fun MonthLine(
    cumulative: List<Long>,
    days: Int,
    budget: Long?,
    selected: Int?,
    onSelect: (Int) -> Unit,
    description: String,
    startLabel: String,
    endLabel: String,
    maxLabel: String,
    modifier: Modifier = Modifier,
) {
    val accent = MaterialTheme.colorScheme.primary
    val marker = MaterialTheme.colorScheme.onBackground
    val paceColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f)
    val budgetColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f)
    val axis = MaterialTheme.colorScheme.outlineVariant
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    val progress = rememberDrawIn(1f)
    val drawn = cumulative.size
    val select by rememberUpdatedState(onSelect)
    // The line only climbs (spending is never negative), so its last point is its top.
    val scaleMax = remember(cumulative, budget) {
        maxOf(cumulative.lastOrNull() ?: 0L, budget ?: 0L, 1L).toFloat()
    }
    Column(modifier.fillMaxWidth()) {
        Row(Modifier.fillMaxWidth().clearAndSetSemantics { }, horizontalArrangement = Arrangement.End) {
            Text(maxLabel, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Spacer(Modifier.height(4.dp))
        Canvas(
            Modifier
                .fillMaxWidth()
                .height(180.dp)
                .semantics { contentDescription = description }
                .pointerInput(drawn, days, rtl) {
                    detectTapGestures(onTap = { o -> select(dayIndexAt(o.x, size.width, days, drawn, rtl)) })
                }
                .pointerInput(drawn, days, rtl) {
                    detectHorizontalDragGestures(
                        onDragStart = { o -> select(dayIndexAt(o.x, size.width, days, drawn, rtl)) },
                        onHorizontalDrag = { change, _ ->
                            change.consume()
                            select(dayIndexAt(change.position.x, size.width, days, drawn, rtl))
                        },
                    )
                }
        ) {
            val top = 8.dp.toPx()
            val bottom = size.height - 2.dp.toPx()
            val h = bottom - top
            fun x(i: Int): Float {
                val f = if (days <= 1) 0f else i.toFloat() / (days - 1)
                return if (rtl) size.width * (1f - f) else size.width * f
            }
            fun y(v: Long): Float = bottom - h * (v.toFloat() / scaleMax)

            drawLine(axis, Offset(0f, bottom), Offset(size.width, bottom), strokeWidth = 1.dp.toPx())
            if (budget != null && budget > 0L) {
                val by = y(budget)
                drawLine(budgetColor, Offset(0f, by), Offset(size.width, by), strokeWidth = 1.dp.toPx())
                drawLine(
                    paceColor,
                    Offset(x(0), y(paceAt(budget, 0, days))),
                    Offset(x(days - 1), by),
                    strokeWidth = 1.5.dp.toPx(),
                    cap = StrokeCap.Round,
                    pathEffect = PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 5.dp.toPx())),
                )
            }
            if (drawn == 0) return@Canvas

            val reveal = size.width * progress.value
            clipRect(
                left = if (rtl) size.width - reveal else 0f,
                right = if (rtl) size.width else reveal,
            ) {
                val area = Path().apply {
                    moveTo(x(0), bottom)
                    for (i in 0 until drawn) lineTo(x(i), y(cumulative[i]))
                    lineTo(x(drawn - 1), bottom)
                    close()
                }
                drawPath(area, accent.copy(alpha = 0.10f))
                if (drawn > 1) {
                    val line = Path().apply {
                        moveTo(x(0), y(cumulative[0]))
                        for (i in 1 until drawn) lineTo(x(i), y(cumulative[i]))
                    }
                    drawPath(line, accent, style = Stroke(width = 2.5.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
                }
                // Where the line stands now: today in the current period, the last day of a past one.
                drawCircle(accent, radius = 3.5.dp.toPx(), center = Offset(x(drawn - 1), y(cumulative[drawn - 1])))
            }

            if (selected != null) {
                val i = selected.coerceIn(0, drawn - 1)
                val sx = x(i)
                val point = Offset(sx, y(cumulative[i]))
                drawLine(marker, Offset(sx, top), Offset(sx, bottom), strokeWidth = 1.5.dp.toPx())
                drawCircle(accent, radius = 4.5.dp.toPx(), center = point)
                drawCircle(marker, radius = 4.5.dp.toPx(), center = point, style = Stroke(width = 1.5.dp.toPx()))
            }
        }
        Spacer(Modifier.height(6.dp))
        Row(Modifier.fillMaxWidth().clearAndSetSemantics { }, horizontalArrangement = Arrangement.SpaceBetween) {
            Text(startLabel, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(endLabel, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/**
 * One thin rounded bar per period for what went out, a dot for what came in, the overall budget
 * as a dashed line. [current] is the running period, in the accent; past ones take the secondary
 * rung. [selected] sits on a recessed column; tapping a bar or its label selects it.
 */
@Composable
internal fun TrendBars(
    spent: List<Long>,
    income: List<Long>,
    labels: List<String>,
    names: List<String>,
    current: Int,
    selected: Int,
    onSelect: (Int) -> Unit,
    budget: Long?,
    description: String,
    modifier: Modifier = Modifier,
) {
    val accent = MaterialTheme.colorScheme.primary
    val past = MaterialTheme.colorScheme.secondary
    val dot = MaterialTheme.colorScheme.onBackground
    // Unlit stubs read against the recessed pick column too, so they take the muted ink, not a fill.
    val unlit = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f)
    val pick = MaterialTheme.colorScheme.surfaceContainerHigh
    val guide = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f)
    val axis = MaterialTheme.colorScheme.outlineVariant
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    val progress = rememberDrawIn(1f)
    val n = spent.size
    val select by rememberUpdatedState(onSelect)
    val scaleMax = remember(spent, income, budget) {
        maxOf(spent.maxOrNull() ?: 0L, income.maxOrNull() ?: 0L, budget ?: 0L, 1L).toFloat()
    }
    Column(modifier.fillMaxWidth()) {
        Canvas(
            Modifier
                .fillMaxWidth()
                .height(200.dp)
                .semantics { contentDescription = description }
                .pointerInput(n, rtl) {
                    detectTapGestures(onTap = { o ->
                        if (n > 0 && size.width > 0) {
                            val slot = (o.x / (size.width.toFloat() / n)).toInt().coerceIn(0, n - 1)
                            select(if (rtl) n - 1 - slot else slot)
                        }
                    })
                }
        ) {
            if (n == 0) return@Canvas
            val slotW = size.width / n
            val barW = minOf(18.dp.toPx(), slotW * 0.6f)
            val top = 10.dp.toPx()
            val bottom = size.height - 1.dp.toPx()
            val h = bottom - top
            val stub = 2.dp.toPx()
            fun cx(i: Int): Float = ((if (rtl) n - 1 - i else i) + 0.5f) * slotW

            if (selected in 0 until n) {
                val w = barW + 20.dp.toPx()
                drawRoundRect(
                    pick,
                    topLeft = Offset(cx(selected) - w / 2f, 0f),
                    size = Size(w, size.height),
                    cornerRadius = CornerRadius(12.dp.toPx()),
                )
            }
            drawLine(axis, Offset(0f, bottom), Offset(size.width, bottom), strokeWidth = 1.dp.toPx())
            for (i in 0 until n) {
                val left = cx(i) - barW / 2f
                val v = spent[i]
                if (v <= 0L) {
                    drawRoundRect(unlit, topLeft = Offset(left, bottom - stub), size = Size(barW, stub), cornerRadius = CornerRadius(stub / 2f))
                } else {
                    val bh = maxOf(h * (v / scaleMax) * progress.value, stub)
                    drawRoundRect(
                        if (i == current) accent else past,
                        topLeft = Offset(left, bottom - bh),
                        size = Size(barW, bh),
                        cornerRadius = CornerRadius(barW / 2f),
                    )
                }
            }
            if (budget != null && budget > 0L) {
                val by = bottom - h * (budget / scaleMax)
                drawLine(
                    guide, Offset(0f, by), Offset(size.width, by), strokeWidth = 1.5.dp.toPx(),
                    pathEffect = PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 5.dp.toPx())),
                )
            }
            for (i in 0 until n) {
                val v = income.getOrElse(i) { 0L }
                if (v > 0L) {
                    drawCircle(dot, radius = 3.5.dp.toPx(), center = Offset(cx(i), bottom - h * (v / scaleMax) * progress.value))
                }
            }
        }
        Spacer(Modifier.height(4.dp))
        Row(Modifier.fillMaxWidth()) {
            labels.forEachIndexed { i, label ->
                val picked = i == selected
                Box(
                    Modifier
                        .weight(1f)
                        .heightIn(min = 48.dp)
                        .bounceClick(label = "Show ${names.getOrElse(i) { label }}", role = Role.Tab) { onSelect(i) }
                        .semantics { this.selected = picked },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        label.uppercase(),
                        style = MaterialTheme.typography.labelMedium,
                        color = if (picked) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                    )
                }
            }
        }
    }
}

/**
 * The period as a heat grid: weekday initials, then one rounded square per day, brighter for
 * more spent against the period's busiest day. Today is ringed; days still ahead are bare outlines
 * and take no tap. Every other day opens its entries through [onDay].
 */
@Composable
internal fun HeatGrid(
    period: BudgetPeriod,
    today: LocalDate,
    weekStartsMonday: Boolean,
    heat: List<Int>,
    cellDescription: (LocalDate, Int) -> String,
    onDay: (LocalDate) -> Unit,
    modifier: Modifier = Modifier,
) {
    val first = if (weekStartsMonday) DayOfWeek.MONDAY else DayOfWeek.SUNDAY
    val weekdays = remember(first) { List(7) { first.plus(it.toLong()) } }
    val lead = remember(period, first) { (period.start.dayOfWeek.value - first.value + 7) % 7 }
    val rows = (lead + period.days + 6) / 7
    Column(modifier.fillMaxWidth()) {
        Row(Modifier.fillMaxWidth().clearAndSetSemantics { }) {
            weekdays.forEach { d ->
                Text(
                    Dates.weekdayInitial(d),
                    modifier = Modifier.weight(1f).padding(bottom = 6.dp),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                )
            }
        }
        for (r in 0 until rows) {
            Row(Modifier.fillMaxWidth()) {
                for (c in 0 until 7) {
                    val index = r * 7 + c - lead
                    if (index in 0 until period.days) {
                        val date = period.start.plusDays(index.toLong())
                        HeatCell(
                            date = date,
                            step = heat.getOrElse(index) { 0 },
                            isToday = date == today,
                            isAhead = date.isAfter(today),
                            description = cellDescription(date, index),
                            onClick = { onDay(date) },
                            modifier = Modifier.weight(1f),
                        )
                    } else {
                        Spacer(Modifier.weight(1f).aspectRatio(1f))
                    }
                }
            }
        }
    }
}

@Composable
private fun HeatCell(
    date: LocalDate,
    step: Int,
    isToday: Boolean,
    isAhead: Boolean,
    description: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val shape = RoundedCornerShape(6.dp)
    val s = step.coerceIn(0, HEAT_ALPHA.size - 1)
    val fill = when {
        isAhead -> Color.Transparent
        s == 0 -> MaterialTheme.colorScheme.surfaceContainerHigh
        else -> MaterialTheme.colorScheme.primary.copy(alpha = HEAT_ALPHA[s])
    }
    val ring = when {
        isToday -> Modifier.border(1.5.dp, MaterialTheme.colorScheme.onBackground, shape)
        isAhead -> Modifier.border(1.dp, MaterialTheme.colorScheme.outline, shape)
        else -> Modifier
    }
    val numberColor = when {
        isAhead || s == 0 -> MaterialTheme.colorScheme.onSurfaceVariant
        s == 4 -> MaterialTheme.colorScheme.onPrimary
        else -> MaterialTheme.colorScheme.onBackground
    }
    // The whole slot is the touch target; the drawn square sits 2dp inside it, so neighbours read
    // 4dp apart while each tap area spans its full column. Where a column is under 48dp wide (a
    // 360dp phone), the slot still stands 48dp tall with the square centred in it.
    Box(
        modifier
            .heightIn(min = 48.dp)
            .then(
                if (isAhead) {
                    Modifier
                } else {
                    // The focus ring runs in the 2dp gap around the square, concentric with it.
                    Modifier.bounceClick(label = "Show entries", role = Role.Button, focusShape = RoundedCornerShape(8.dp), onClick = onClick)
                },
            )
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .padding(2.dp)
                .clip(shape)
                .background(fill)
                .then(ring),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                date.dayOfMonth.toString(),
                modifier = Modifier.clearAndSetSemantics { },
                style = MaterialTheme.typography.labelSmall,
                color = numberColor,
            )
        }
    }
}

/**
 * Lets a grid of [columns] run past its parent's padding on both sides: by [min] at least, and
 * further, up to [max], until each column is [column] wide. A 411dp phone needs only [min]; on a
 * 360dp phone even [max] leaves the columns short of [column], so the cells keep their height
 * and take the width there is. The panel's own clip still bounds it.
 */
internal fun Modifier.bleed(min: Dp, max: Dp, columns: Int, column: Dp): Modifier = layout { measurable, constraints ->
    if (!constraints.hasBoundedWidth) {
        val placeable = measurable.measure(constraints)
        return@layout layout(placeable.width, placeable.height) { placeable.place(0, 0) }
    }
    val wanted = column.roundToPx() * columns - constraints.maxWidth
    val side = ((wanted + 1) / 2).coerceIn(min.roundToPx(), max.roundToPx())
    val extra = side * 2
    val placeable = measurable.measure(
        constraints.copy(minWidth = constraints.minWidth + extra, maxWidth = constraints.maxWidth + extra)
    )
    val width = (placeable.width - extra).coerceIn(constraints.minWidth, constraints.maxWidth)
    layout(width, placeable.height) { placeable.place(-side, 0) }
}
