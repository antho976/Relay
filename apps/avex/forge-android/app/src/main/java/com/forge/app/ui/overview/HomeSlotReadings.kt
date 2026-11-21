package com.forge.app.ui.overview

import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.durationMinutes
import com.forge.app.data.repo.StatsRepository
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeightDelta
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.program.Program
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.TextStyle
import java.time.temporal.ChronoUnit
import java.util.Locale
import kotlin.math.abs

/** A lift that moved less than this either way reads as matched: a stub, not a bar. */
internal const val MATCHED_PCT = 0.5f

/**
 * The last session as a slot reading: its heaviest set as the figure ("275×5 squat"), each lift
 * against its own last time as bars, and "Legs A · 4/5 up" as the change line. Labelled by when it
 * was: "Today", "Yesterday", a weekday within the week, or "Last session" beyond that.
 */
internal fun lastSessionSlot(
    session: Session,
    lines: List<StatsRepository.SessionExerciseLine>,
    deltas: List<StatsRepository.LiftDelta>,
    unit: WeightUnit,
    todayStartMs: Long,
    zone: ZoneId = ZoneId.systemDefault(),
): SlotReading {
    val today = Instant.ofEpochMilli(todayStartMs).atZone(zone).toLocalDate()
    val day = Instant.ofEpochMilli(session.startedAt).atZone(zone).toLocalDate()
    val label = when (val ago = ChronoUnit.DAYS.between(day, today)) {
        0L -> "Today"
        1L -> "Yesterday"
        in 2L..6L -> day.dayOfWeek.getDisplayName(TextStyle.SHORT, Locale.getDefault())
        else -> "Last session"
    }
    val title = Program.dayDisplayName(session.dayKey)
    val compared = deltas.mapNotNull { it.pct }
    val up = compared.count { it >= MATCHED_PCT }
    val change = if (compared.isEmpty()) title else "$title · $up/${compared.size} up"
    // A first session (or one of all-new lifts) has nothing to compare: each lift's sets stand as its
    // bar instead, so the mark still shows the session's shape rather than a bare rule.
    val bars = if (compared.isEmpty()) lines.map { it.setCount.toFloat() }.takeIf { b -> b.any { it > 0f } }
        else deltas.map { d -> d.pct?.takeIf { abs(it) >= MATCHED_PCT } ?: d.pct?.let { 0f } }

    val top = lines.filter { (it.topWeightLb ?: 0.0) > 0.0 }.maxByOrNull { it.topWeightLb!! }
    return if (top != null) {
        val weight = weightInputValue(top.topWeightLb!!, unit)
        // "275×5 squat" fits half the page at the figure's size; anything longer drops the name.
        val reps = "×${top.topReps ?: 0}"
        val name = shortLiftName(top.exerciseName)?.takeIf { weight.length + reps.length + 1 + it.length <= 12 }
        SlotReading(
            figure = weight,
            unit = reps + (name?.let { " $it" } ?: ""),
            bars = bars,
            change = change,
            changeMoved = up > 0,
            label = label,
            picker = label,
            description = "$label, $title: top set ${top.exerciseName} $weight ${unitLabel(unit)} for ${top.topReps ?: 0}" +
                if (compared.isEmpty()) "" else ", $up of ${compared.size} lifts up on last time",
        )
    } else {
        val minutes = session.durationMinutes() ?: 0
        SlotReading(
            figure = "$minutes",
            unit = " min",
            bars = bars,
            change = change,
            changeMoved = up > 0,
            label = label,
            picker = label,
            description = "$label, $title: $minutes minutes",
        )
    }
}

/**
 * The lift's name short enough to sit after the figure at half size: the last word when the whole
 * name is long ("Barbell Back Squat" → "squat"), lower case as a unit reads; null when even that does
 * not fit, so the figure stands alone.
 */
internal fun shortLiftName(name: String): String? {
    val short = compactLiftName(name)
    val word = if (short.length <= 8) short else short.substringAfterLast(' ')
    return word.lowercase().takeIf { it.isNotBlank() && it.length <= 9 }
}

private val PARENTHETICAL = Regex("""\s*\([^)]*\)""")

private val EQUIPMENT_WORDS = setOf(
    "barbell", "dumbbell", "db", "kb", "kettlebell", "cable", "ez-bar", "ez", "machine", "smith", "trap-bar", "landmine",
)

/**
 * A lift's name without what it is done with: "Cable Tricep Pushdown" → "Tricep Pushdown",
 * "Calf Raise (machine)" → "Calf Raise". Home's rows print the lift beside a duration in a third of
 * the width; the equipment is on the session page.
 */
internal fun compactLiftName(name: String): String {
    val words = name.replace(PARENTHETICAL, "").trim().split(' ').filter { it.isNotBlank() }
    val kept = words.dropWhile { it.lowercase() in EQUIPMENT_WORDS }
    return (kept.ifEmpty { words }).joinToString(" ")
}

/**
 * Body weight from the weigh-ins (newest first): the latest as the figure, the last ten as a line,
 * and the change against the weigh-in closest to 30 days before it.
 */
internal fun bodyWeightSlot(entries: List<BodyweightEntry>, unit: WeightUnit): SlotReading? {
    val latest = entries.firstOrNull() ?: return null
    val latestDate = runCatching { LocalDate.parse(latest.dateKey) }.getOrNull() ?: return null
    val target = latestDate.minusDays(30)
    val then = entries.drop(1)
        .mapNotNull { e -> runCatching { LocalDate.parse(e.dateKey) }.getOrNull()?.let { it to e } }
        .filter { (d, _) -> d <= latestDate.minusDays(21) }
        .minByOrNull { (d, _) -> abs(ChronoUnit.DAYS.between(d, target)) }?.second
    val delta = then?.let { latest.weightLb - it.weightLb }
    val change = delta?.let {
        if (abs(it) < 0.5) "Steady over 30d"
        else "${if (it < 0) "↓" else "↑"} ${formatWeightDelta(abs(it), unit)} in 30d"
    }
    val figure = weightInputValue(latest.weightLb, unit)
    return SlotReading(
        figure = figure,
        unit = " ${unitLabel(unit)}",
        spark = entries.take(10).reversed().map { it.weightLb.toFloat() },
        change = change,
        changeMoved = delta != null && abs(delta) >= 0.5,
        description = "Body weight $figure ${unitLabel(unit)}" + (change?.let { ", ${it.lowercase()}" } ?: ""),
    )
}

/** How many weeks the week-volume reading's bars span, this one last. */
private const val VOLUME_WEEKS = 8

/**
 * This week's lifted volume: the figure so far, one bar per week for the last eight (this week the
 * newest), and the change on last week. Null until a session with volume has been logged in that span.
 */
internal fun weekVolumeSlot(
    sessions: List<Session>,
    unit: WeightUnit,
    todayStartMs: Long,
    firstDayMonday: Boolean,
    zone: ZoneId = ZoneId.systemDefault(),
): SlotReading? {
    val today = Instant.ofEpochMilli(todayStartMs).atZone(zone).toLocalDate()
    val firstDay = if (firstDayMonday) java.time.DayOfWeek.MONDAY else java.time.DayOfWeek.SUNDAY
    val weekStart = today.with(java.time.temporal.TemporalAdjusters.previousOrSame(firstDay))
    val weeks = DoubleArray(VOLUME_WEEKS)
    sessions.forEach { s ->
        val volume = s.totalVolumeLb ?: return@forEach
        val day = Instant.ofEpochMilli(s.startedAt).atZone(zone).toLocalDate()
        val ago = ChronoUnit.WEEKS.between(day.with(java.time.temporal.TemporalAdjusters.previousOrSame(firstDay)), weekStart).toInt()
        if (ago in 0 until VOLUME_WEEKS) weeks[VOLUME_WEEKS - 1 - ago] += volume
    }
    if (weeks.all { it <= 0.0 }) return null
    val now = weeks.last()
    val last = weeks[VOLUME_WEEKS - 2]
    val figure = com.forge.app.domain.units.formatVolumeCompact(now, unit, withUnit = false)
    val change = when {
        weeks.dropLast(1).all { it <= 0.0 } -> "Your first week"
        last <= 0.0 -> "${com.forge.app.domain.units.formatVolumeCompact(weeks.sum(), unit, withUnit = false)} in ${VOLUME_WEEKS} wk"
        else -> "Last week ${com.forge.app.domain.units.formatVolumeCompact(last, unit, withUnit = false)}"
    }
    return SlotReading(
        figure = figure,
        unit = " ${unitLabel(unit)}",
        bars = weeks.map { it.toFloat() },
        change = change,
        changeMoved = last > 0.0 && now >= last,
        description = "This week, $figure ${unitLabel(unit)} lifted; ${change.lowercase()}",
    )
}
