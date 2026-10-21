package com.forge.app.ui.profile

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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.profile.state.ProfileUiState
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.ProfileColors
import com.forge.app.ui.theme.ProfileType
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle

/*
 * Overview (pf1): all time — what you have lifted, then the counts — the lifetime curve under it,
 * then Consistency as one month of days with a small month switcher on the right. The month is the
 * grid that used to be Stats' "Days" lens; a trained day opens the same day sheet.
 */

private val CELL_SHAPE = RoundedCornerShape(4.dp)

@Composable
internal fun ProfileOverviewTab(
    state: ProfileUiState,
    unit: WeightUnit,
    today: LocalDate,
    nowMs: Long,
    planOn: (LocalDate) -> Boolean?,
    onVacation: (LocalDate) -> Boolean,
    onDayTap: (LocalDate) -> Unit,
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val locale = currentLocale()

    // ── All time ──
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
        OpenLabel("All time")
        if (state.sinceLabel.isNotBlank()) OpenLabel("Since ${state.sinceLabel}")
    }
    Spacer(Modifier.height(14.dp))
    val lifted = remember(state.lifetimeVolumeSeriesLb, state.lifetimeVolumeTimesMs, nowMs, unit) {
        liftedReading(state.lifetimeVolumeSeriesLb, state.lifetimeVolumeTimesMs, nowMs, unit)
    }
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
        FittedFigure(
            compactFigure(lifted.total), " ${unitLabel(unit)} lifted",
            listOf(OpenType.figureL to ProfileType.liftedUnit, OpenType.figureM to ProfileType.liftedUnit),
            Modifier.weight(1f)
        )
        val moved = lifted.last30 >= 1
        OpenLabel(
            if (moved) "↑ ${compactFigure(lifted.last30)} in 30 days" else "0 in 30 days",
            style = OpenType.monoXS,
            color = if (moved) ink else muted,
            modifier = Modifier.padding(start = 12.dp, bottom = 10.dp)
        )
    }
    Spacer(Modifier.height(26.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
        CountColumn(state.totalSessions, "Workouts", state.workoutsThisWeek, "this week", Modifier.weight(1f))
        CountColumn(state.totalSets, "Sets", state.setsThisWeek, "this week", Modifier.weight(1f))
        CountColumn(state.totalPrs, "PRs", state.prsThisMonth, "this month", Modifier.weight(1f))
    }
    Spacer(Modifier.height(20.dp))
    val curve = remember(state.lifetimeVolumeSeriesLb, state.lifetimeVolumeTimesMs, nowMs, locale) {
        lifetimeCurve(state.lifetimeVolumeSeriesLb, state.lifetimeVolumeTimesMs, nowMs, ZoneId.systemDefault(), locale)
    }
    LifetimeArea(curve, "${compactFigure(lifted.total)} ${unitLabel(unit)} lifted all time")
    if (curve.points.isEmpty()) {
        Spacer(Modifier.height(8.dp))
        OpenLabel("Two workouts start the lifetime line", style = OpenType.monoXS)
    }

    // ── Consistency ──
    Spacer(Modifier.height(52.dp))
    ConsistencyMonth(state, today, planOn, onVacation, onDayTap)
}

@Composable
private fun CountColumn(value: Int, label: String, change: Int, window: String, modifier: Modifier) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(
        modifier.semantics(mergeDescendants = true) {},
        verticalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        Text(compactFigure(value.toDouble()), style = ProfileType.count, color = ink)
        OpenLabel(label)
        OpenLabel(
            if (change > 0) "↑ $change $window" else "0 $window",
            style = OpenType.monoXS,
            color = if (change > 0) ink else muted
        )
    }
}

/**
 * The lifetime curve: the running total over the last year as a red line over a faint red area,
 * with month names under it. With fewer than two workouts it is the bare baseline.
 */
@Composable
private fun LifetimeArea(curve: LifetimeCurve, description: String) {
    val accent = MaterialTheme.colorScheme.primary
    val measurer = rememberTextMeasurer()
    Canvas(Modifier.fillMaxWidth().height(64.dp).clearAndSetSemantics { contentDescription = description }) {
        val labelRoom = 18.dp.toPx()
        val bottom = size.height - labelRoom
        val pts = curve.points
        if (pts.size < 2) {
            drawRect(OpenColors.Rule, topLeft = Offset(0f, bottom - 1.dp.toPx()), size = androidx.compose.ui.geometry.Size(size.width, 1.dp.toPx()))
            return@Canvas
        }
        val inset = 14.dp.toPx()
        val w = size.width - 2 * inset
        val lo = pts.minOf { it.y }
        val hi = pts.maxOf { it.y }
        val span = (hi - lo).takeIf { it > 0f } ?: 1f
        val top = 4.dp.toPx()
        // The curve starts a third of the way up, so an all-time total reads as accumulated rather
        // than as a climb from nothing.
        val floorY = bottom - (bottom - top) * 0.25f
        val offsets = pts.map { Offset(inset + it.x * w, floorY - (it.y - lo) / span * (floorY - top)) }
        val line = Path().apply { offsets.forEachIndexed { i, p -> if (i == 0) moveTo(p.x, p.y) else lineTo(p.x, p.y) } }
        val area = Path().apply {
            addPath(line)
            lineTo(offsets.last().x, bottom)
            lineTo(offsets.first().x, bottom)
            close()
        }
        drawPath(area, accent.copy(alpha = 0.15f))
        drawPath(line, accent, style = Stroke(2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
        curve.months.forEach { (x, name) ->
            val t = measurer.measure(name, OpenType.chartLabel.copy(color = OpenColors.Glyph))
            val cx = (inset + x * w - t.size.width / 2f).coerceIn(0f, size.width - t.size.width)
            drawText(t, topLeft = Offset(cx, bottom + 5.dp.toPx()))
        }
    }
}

/** The month grid with its switcher, its plan line and its key. */
@Composable
private fun ConsistencyMonth(
    state: ProfileUiState,
    today: LocalDate,
    planOn: (LocalDate) -> Boolean?,
    onVacation: (LocalDate) -> Boolean,
    onDayTap: (LocalDate) -> Unit,
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val locale = currentLocale()
    val thisMonth = YearMonth.from(today)
    var monthsBack by rememberSaveable { mutableIntStateOf(0) }
    val month = thisMonth.minusMonths(monthsBack.toLong())
    // Back stops at the oldest month with anything in it; forward stops at this one.
    val oldest = remember(state.activityByDay, thisMonth) {
        state.activityByDay.keys.minOrNull()?.let { YearMonth.from(LocalDate.ofEpochDay(it)) }?.coerceAtMost(thisMonth) ?: thisMonth
    }
    val canBack = month.isAfter(oldest)
    val canForward = monthsBack > 0

    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
        OpenLabel("Consistency")
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            MonthStep(OpenIcons.ChevronLeft, "Earlier month", canBack) { monthsBack++ }
            val name = month.month.getDisplayName(TextStyle.FULL, locale)
            OpenLabel(
                if (month.year == today.year) name else "${month.month.getDisplayName(TextStyle.SHORT, locale)} ${month.year}",
                color = ink,
                modifier = Modifier.semantics { contentDescription = "${month.month.getDisplayName(TextStyle.FULL, locale)} ${month.year}" }
            )
            MonthStep(OpenIcons.ChevronRight, "Later month", canForward) { monthsBack-- }
        }
    }
    Spacer(Modifier.height(10.dp))
    val weeks = remember(state.activityByDay, state.deloadDays, month, today, planOn, onVacation) {
        monthGrid(month, today, state.activityByDay, state.deloadDays, planOn, onVacation)
    }
    val run = remember(state.activityByDay, today, planOn, onVacation) {
        weeksOnPlan(today, state.activityByDay, planOn, onVacation)
    }
    val line = remember(state.activityByDay, month, today, planOn, onVacation, run) {
        consistencyLine(month, today, state.activityByDay, planOn, onVacation, run)
    }
    OpenLabel(line, style = OpenType.monoXS, color = ink)
    Spacer(Modifier.height(22.dp))

    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            // Monday first, as every calendar in the app is.
            listOf(1, 2, 3, 4, 5, 6, 7).forEach { dow ->
                Text(
                    java.time.DayOfWeek.of(dow).getDisplayName(TextStyle.NARROW, locale),
                    style = ProfileType.weekday,
                    color = OpenColors.Glyph,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.weight(1f).clearAndSetSemantics { }
                )
            }
        }
        val fmt = remember(locale) { DateTimeFormatter.ofPattern("MMMM d", locale) }
        weeks.forEach { week ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                week.forEach { day ->
                    if (day == null) Spacer(Modifier.weight(1f).height(30.dp))
                    else DayCell(day, fmt, onDayTap, Modifier.weight(1f))
                }
            }
        }
    }
    Spacer(Modifier.height(12.dp))
    val missed = weeks.any { w -> w.any { it?.mark == DayMark.MISSED } }
    OpenLabel(
        "Red trained · faded deload" + (if (missed) " · edged missed" else "") + " · tap a day",
        style = OpenType.monoXS
    )
}

@Composable
private fun MonthStep(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, enabled: Boolean, onClick: () -> Unit) {
    Box(
        Modifier
            .touchSlop(vertical = 17.dp, horizontal = 10.dp)
            .then(
                if (enabled) Modifier.semantics { contentDescription = label }.clickableLabeled(label, onClick = onClick)
                else Modifier.semantics { contentDescription = label; disabled() }
            )
            .padding(vertical = 17.dp, horizontal = 10.dp)
    ) {
        Icon(
            icon,
            contentDescription = null,
            tint = if (enabled) MaterialTheme.colorScheme.onBackground else OpenColors.Track,
            modifier = Modifier.size(14.dp)
        )
    }
}

@Composable
private fun DayCell(day: MonthDay, fmt: DateTimeFormatter, onDayTap: (LocalDate) -> Unit, modifier: Modifier) {
    val accent = MaterialTheme.colorScheme.primary
    val reading = buildString {
        append(day.date.format(fmt))
        when (day.mark) {
            DayMark.TRAINED, DayMark.DELOAD -> {
                append(if (day.count == 1) ", 1 session" else ", ${day.count} sessions")
                if (day.mark == DayMark.DELOAD) append(", deload")
            }
            DayMark.REST -> append(", planned rest")
            DayMark.MISSED -> append(", planned, not trained")
            DayMark.TODAY -> append(", today")
            DayMark.FUTURE -> append(", ahead")
            DayMark.OPEN -> append(", not trained")
        }
        if (day.isToday && day.mark != DayMark.TODAY) append(", today")
    }
    val lit = day.mark == DayMark.TRAINED || day.mark == DayMark.DELOAD
    Box(
        modifier
            .height(30.dp)
            .clip(CELL_SHAPE)
            .then(
                when (day.mark) {
                    DayMark.TRAINED -> Modifier.background(accent)
                    DayMark.DELOAD -> Modifier.background(accent.copy(alpha = 0.6f))
                    DayMark.REST, DayMark.OPEN -> Modifier.background(ProfileColors.DayPast)
                    DayMark.MISSED -> Modifier.background(ProfileColors.DayPast).border(1.dp, OpenColors.Rule, CELL_SHAPE)
                    DayMark.TODAY -> Modifier.border(1.5.dp, accent, CELL_SHAPE)
                    DayMark.FUTURE -> Modifier.border(1.dp, OpenColors.Unreached, CELL_SHAPE)
                }
            )
            .then(
                if (lit) Modifier.semantics { contentDescription = reading }.clickableLabeled("Open $reading") { onDayTap(day.date) }
                else Modifier.clearAndSetSemantics { contentDescription = reading }
            )
    )
}
