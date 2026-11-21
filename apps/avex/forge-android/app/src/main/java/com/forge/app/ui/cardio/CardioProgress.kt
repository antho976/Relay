package com.forge.app.ui.cardio

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioPaceWindow
import com.forge.app.domain.cardio.cardioBests
import com.forge.app.domain.cardio.cardioPaceWindow
import com.forge.app.domain.cardio.formatPaceSec
import com.forge.app.domain.cardio.paceSecPerUnit
import com.forge.app.domain.cardio.readsAsSpeed
import com.forge.app.domain.cardio.speedPerHour
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.ui.cardio.state.CardioUiState
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenMeter
import com.forge.app.ui.common.OpenSlotLabel
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.common.window.DropdownMenu
import com.forge.app.ui.goals.customGoalTitle
import com.forge.app.ui.goals.goalCaption
import com.forge.app.ui.theme.CardioOpenType
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenType
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle as JavaTextStyle
import java.util.Locale
import kotlin.math.roundToInt

/**
 * PROGRESS — one activity's pace (a ride's speed) over twelve weeks, its bests, and the cardio goals.
 * The activity picker chooses among those with at least two sessions that logged a distance.
 */
@Composable
internal fun CardioProgressTab(state: CardioUiState, zone: ZoneId, onOpenGoals: () -> Unit) {
    val customs = LocalCardioTypes.current
    val series = state.cardioPaceSeries
    var picked by rememberSaveable { mutableStateOf<String?>(null) }
    val selected = series.firstOrNull { it.typeCode == picked } ?: series.firstOrNull()
    Column {
        Spacer(Modifier.height(10.dp))
        if (selected == null) {
            CardioHead("Pace · 12 weeks")
            Spacer(Modifier.height(14.dp))
            CardioBodyLine("0 paced sessions yet. Log two with a distance and your pace over time reads out here.")
        } else {
            val name = CardioActivity.resolve(selected.typeCode, customs).displayName
            val window = remember(selected, state.todayStartMs, state.useMiles) {
                cardioPaceWindow(selected, state.todayStartMs.takeIf { it > 0 } ?: System.currentTimeMillis(), state.useMiles)
            }
            var picking by remember { mutableStateOf(false) }
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.weight(1f)) {
                    OpenSlotLabel(name, onClick = { picking = true })
                    DropdownMenu(expanded = picking, onDismissRequest = { picking = false }) {
                        series.forEach { s ->
                            DropdownMenuItem(
                                text = { Text(CardioActivity.resolve(s.typeCode, customs).displayName, style = OpenType.body) },
                                onClick = { picked = s.typeCode; picking = false }
                            )
                        }
                    }
                }
                OpenLabel(
                    "${if (window?.speed == true) "Speed" else "Pace"} · ${if (window?.allTime == true) "all time" else "12 weeks"}"
                )
            }
            if (window != null) PaceSection(window, state.useMiles, zone)
            Spacer(Modifier.height(CARDIO_SECTION))
            BestsSection(state.entries, selected.typeCode, name, state.useMiles, zone, state.todayStartMs)
        }
        Spacer(Modifier.height(CARDIO_SECTION))
        GoalsSection(state.cardioGoals, state.todayStartMs, state.useMiles, onOpenGoals)
    }
}

/** A value in the window's own reading: "9:31" for a pace, "14.2" for a speed. */
private fun readingText(v: Double, speed: Boolean): String =
    if (speed) String.format(Locale.US, "%.1f", v) else formatPaceSec(v.roundToInt())

/** What [CardioPaceSection] prints, worked out without a composition so a test can pin it. */
internal data class PaceRead(
    val now: String,
    val unit: String,
    val change: String?,
    val changeMoved: Boolean,
    val startLabel: String,
    val endLabel: String,
    val xLabels: List<Pair<Float, String>>,
    /** (x 0..1, y) with up = faster. */
    val points: List<Pair<Float, Float>>,
)

internal fun paceRead(w: CardioPaceWindow, useMiles: Boolean, zone: ZoneId, locale: Locale): PaceRead {
    val unit = if (w.speed) " ${speedUnitLabel(useMiles)}" else " /${distanceUnitLabel(useMiles)}"
    val span = (w.endMs - w.startMs).coerceAtLeast(1L).toFloat()
    fun date(ms: Long): LocalDate = Instant.ofEpochMilli(ms).atZone(zone).toLocalDate()
    val points = w.points.map { (ms, v) -> ((ms - w.startMs) / span).coerceIn(0f, 1f) to (if (w.speed) v else -v).toFloat() }
    val now = w.points.last().second
    // The baseline is the first calendar month on the line, read as its average, and named.
    val firstMonth = date(w.points.first().first).withDayOfMonth(1)
    val base = w.points.filter { date(it.first).withDayOfMonth(1) == firstMonth }
    val lastMonth = date(w.points.last().first).withDayOfMonth(1)
    val baseValue = base.map { it.second }.average()
    val monthName = firstMonth.month.getDisplayName(JavaTextStyle.FULL, locale)
    val change = if (firstMonth == lastMonth || base.size == w.points.size) null else {
        val better = if (w.speed) now > baseValue else now < baseValue
        val gap = kotlin.math.abs(now - baseValue)
        val gapText = if (w.speed) String.format(Locale.US, "%.1f", gap) else formatPaceSec(gap.roundToInt())
        when {
            (if (w.speed) gap < 0.05 else gap < 0.5) -> "Level with $monthName"
            better -> "${if (w.speed) "↑" else "↓"} $gapText faster than $monthName"
            else -> "${if (w.speed) "↓" else "↑"} $gapText slower than $monthName"
        }
    }
    val monthTicks = mutableListOf<Pair<Float, String>>()
    var m = date(w.startMs).withDayOfMonth(1).plusMonths(1)
    val endDate = date(w.endMs)
    val monthFmt = DateTimeFormatter.ofPattern("MMM", locale)
    while (!m.isAfter(endDate)) {
        val f = (m.atStartOfDay(zone).toInstant().toEpochMilli() - w.startMs) / span
        if (f in 0f..0.86f) monthTicks += f to m.format(monthFmt)
        m = m.plusMonths(1)
    }
    // The window's own start month names the left edge only when no month begins near it.
    if ((monthTicks.firstOrNull()?.first ?: 1f) > 0.2f) monthTicks.add(0, 0f to date(w.startMs).format(monthFmt))
    monthTicks += 1f to "Now"
    val shortMonth = date(w.points.first().first).format(monthFmt)
    return PaceRead(
        now = readingText(now, w.speed),
        unit = unit,
        change = change,
        changeMoved = change != null && !change.startsWith("Level"),
        startLabel = "$shortMonth ${readingText(w.points.first().second, w.speed)}",
        endLabel = "Up is faster · now ${readingText(now, w.speed)}",
        xLabels = monthTicks,
        points = points,
    )
}

@Composable
private fun PaceSection(w: CardioPaceWindow, useMiles: Boolean, zone: ZoneId) {
    val locale = currentLocale()
    val read = remember(w, useMiles, zone, locale) { paceRead(w, useMiles, zone, locale) }
    Spacer(Modifier.height(10.dp))
    Text(
        cardioFigure(read.now, read.unit, OpenType.figureXLUnit),
        style = OpenType.figureXL,
        color = MaterialTheme.colorScheme.onBackground,
        modifier = Modifier.clearAndSetSemantics { contentDescription = "Latest ${read.now}${read.unit}" }
    )
    if (read.change != null) {
        Spacer(Modifier.height(8.dp))
        CardioChange(read.change, read.changeMoved)
    }
    Spacer(Modifier.height(16.dp))
    CardioLineChart(
        points = read.points,
        xLabels = read.xLabels,
        description = "${if (w.speed) "Speed" else "Pace"} over time, higher is faster. ${read.startLabel}, now ${read.now}${read.unit}."
    )
    Spacer(Modifier.height(6.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        CardioChange(read.startLabel, moved = false, modifier = Modifier.weight(1f))
        CardioChange(read.endLabel, moved = true)
    }
}

/** The selected activity's bests: fastest (or top speed), longest, and its biggest week. */
@Composable
private fun BestsSection(
    entries: List<CardioEntry>,
    typeCode: String,
    name: String,
    useMiles: Boolean,
    zone: ZoneId,
    todayStartMs: Long,
) {
    val locale = currentLocale()
    val bests = remember(entries, typeCode, zone) { cardioBests(entries, typeCode, zone) } ?: return
    val unit = distanceUnitLabel(useMiles)
    val dateFmt = remember(locale) { DateTimeFormatter.ofPattern("MMM d", locale) }
    val today = Instant.ofEpochMilli(todayStartMs.takeIf { it > 0 } ?: System.currentTimeMillis()).atZone(zone).toLocalDate()
    fun date(ms: Long) = Instant.ofEpochMilli(ms).atZone(zone).toLocalDate()
    fun thisMonth(ms: Long?) = ms != null && date(ms).withDayOfMonth(1) == today.withDayOfMonth(1)
    fun line(ms: Long?): Pair<String, Boolean> =
        if (ms == null) "" to false else if (thisMonth(ms)) "${date(ms).format(dateFmt)} · new" to true else date(ms).format(dateFmt) to false
    val speed = readsAsSpeed(typeCode)
    val fastest = bests.fastest
    val fastestText = fastest?.let {
        if (speed) speedPerHour(it.durationMin, it.distanceKm, useMiles)?.let { s -> String.format(Locale.US, "%.1f", s) }
        else paceSecPerUnit(it.durationMin, it.distanceKm, useMiles)?.let(::formatPaceSec)
    } ?: "0"
    val newCount = listOf(fastest?.date, bests.longest?.date, bests.bestWeekStartMs).count { thisMonth(it) }
    CardioHead("Bests · $name", if (newCount == 0) "None new this month" else "$newCount this month", rightInk = newCount > 0)
    Spacer(Modifier.height(14.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
        val (fLine, fNew) = line(fastest?.date)
        CardioReading(
            label = if (speed) "Top speed" else "Best pace",
            figure = cardioFigure(fastestText, if (speed) " ${speedUnitLabel(useMiles)}" else "", CardioOpenType.figureMUnit),
            change = fLine, moved = fNew,
            description = "${if (speed) "Top speed" else "Best pace"} $fastestText, $fLine",
            modifier = Modifier.weight(1f)
        )
        val longestKm = bests.longest?.distanceKm ?: 0.0
        val (lLine, lNew) = line(bests.longest?.date)
        CardioReading(
            label = "Longest",
            figure = cardioFigure(distanceNumber(longestKm, useMiles), " $unit", CardioOpenType.figureMUnit),
            change = lLine, moved = lNew,
            description = "Longest ${distanceNumber(longestKm, useMiles)} $unit, $lLine",
            modifier = Modifier.weight(1f)
        )
        val (wLine, wNew) = line(bests.bestWeekStartMs)
        CardioReading(
            label = "Best week",
            figure = cardioFigure(distanceNumber(bests.bestWeekKm, useMiles), " $unit", CardioOpenType.figureMUnit),
            change = wLine, moved = wNew,
            description = "Best week ${distanceNumber(bests.bestWeekKm, useMiles)} $unit, week of $wLine",
            modifier = Modifier.weight(1f)
        )
    }
}

/** The cardio goals (distance and minutes), each as its name, where it stands, and its meter. */
@Composable
private fun GoalsSection(
    goals: List<ExtendedGoalRepository.Progress>,
    todayStartMs: Long,
    useMiles: Boolean,
    onOpenGoals: () -> Unit,
) {
    val settings = LocalForgeSettings.current
    CardioLinkHead(
        label = "Goals",
        action = if (goals.isEmpty()) "Set one" else "View all",
        actionLabel = "Open goals",
        onAction = onOpenGoals
    )
    Spacer(Modifier.height(12.dp))
    if (goals.isEmpty()) {
        CardioBodyLine("0 cardio goals. A distance or minutes goal shows its progress here.")
        return
    }
    val shown = remember(goals) {
        goals.sortedWith(compareBy<ExtendedGoalRepository.Progress> { it.achieved }.thenByDescending { it.fraction }).take(2)
    }
    val now = remember(shown, todayStartMs) { System.currentTimeMillis() }
    val unit = distanceUnitLabel(useMiles)
    Column(verticalArrangement = Arrangement.spacedBy(28.dp)) {
        shown.forEach { g ->
            val distance = g.metric == GoalMetric.CARDIO_DISTANCE
            val value = if (distance) distanceNumber(g.currentValue, useMiles) else "${g.currentValue.toInt()}"
            val target = if (distance) "${distanceNumber(g.targetValue, useMiles)} $unit" else "${g.targetValue.toInt()} min"
            val caption = listOfNotNull(
                "Of $target",
                goalCaption(g.achieved, g.metric, g.period, g.baselineValue, settings.weightUnit, now)
            ).joinToString(" · ")
            Column(Modifier.clearAndSetSemantics { contentDescription = "${customGoalTitle(g)}: $value of $target" }) {
                Text(customGoalTitle(g), style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground)
                Spacer(Modifier.height(6.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.Bottom) {
                    Text(
                        cardioFigure(value, if (distance) " $unit" else " min", CardioOpenType.figureMUnit),
                        style = OpenType.figureM,
                        color = MaterialTheme.colorScheme.onBackground
                    )
                    CardioChange(caption, moved = g.achieved, modifier = Modifier.weight(1f))
                }
                Spacer(Modifier.height(10.dp))
                OpenMeter(g.fraction)
            }
        }
    }
}
