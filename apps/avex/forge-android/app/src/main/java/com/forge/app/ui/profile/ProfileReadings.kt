package com.forge.app.ui.profile

import com.forge.app.data.db.entities.BodyFatEntry
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.LeanMassEntry
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.measurement.BodyMeasurementType
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.formatWeightDelta
import com.forge.app.domain.units.lengthUnitLabel
import com.forge.app.domain.units.toDisplayLength
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import java.time.temporal.ChronoUnit
import java.time.temporal.TemporalAdjusters
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToInt

/*
 * The Profile page's derived readings, as pure functions so they are tested without a screen. The
 * page (ProfileOpen.kt) only lays out what these return.
 */

private const val DAY_MS = 86_400_000L

/** "2.9M", "248k", "4,830", "64": a big count read at a glance. */
internal fun compactFigure(v: Double): String {
    val a = abs(v)
    return when {
        a >= 1_000_000 -> String.format(Locale.US, "%.1f", v / 1_000_000).trimEnd('0').trimEnd('.') + "M"
        a >= 100_000 -> "${(v / 1000).roundToInt()}k"
        a >= 10_000 -> String.format(Locale.US, "%.1f", v / 1000).trimEnd('0').trimEnd('.') + "k"
        else -> String.format(Locale.US, "%,d", v.roundToInt())
    }
}

// ── All time ──────────────────────────────────────────────────────────────────────────────────

/** What all time adds up to: the lifted total and what the last 30 days added, in the display unit. */
internal data class LiftedReading(val total: Double, val last30: Double)

internal fun liftedReading(seriesLb: List<Double>, timesMs: List<Long>, nowMs: Long, unit: WeightUnit): LiftedReading {
    val total = seriesLb.lastOrNull() ?: 0.0
    val cutoff = nowMs - 30 * DAY_MS
    // The running total just before the window opened; zero when the window holds all of history.
    val before = if (timesMs.size == seriesLb.size) {
        seriesLb.indices.lastOrNull { timesMs[it] < cutoff }?.let { seriesLb[it] } ?: 0.0
    } else total
    return LiftedReading(toDisplayWeight(total, unit), toDisplayWeight(total - before, unit))
}

/** One point of the lifetime curve: [x] from 0 (the window's start) to 1 (now), [y] the running total. */
internal data class CurvePoint(val x: Float, val y: Float)

/** The lifetime curve over the last year (or since the first session, if that is sooner), with month names under it. */
internal data class LifetimeCurve(val points: List<CurvePoint>, val months: List<Pair<Float, String>>)

internal fun lifetimeCurve(
    seriesLb: List<Double>,
    timesMs: List<Long>,
    nowMs: Long,
    zone: ZoneId,
    locale: Locale,
): LifetimeCurve {
    if (seriesLb.size < 2 || timesMs.size != seriesLb.size) return LifetimeCurve(emptyList(), emptyList())
    val start = maxOf(timesMs.first(), nowMs - 365 * DAY_MS)
    val span = (nowMs - start).coerceAtLeast(DAY_MS).toFloat()
    val inside = seriesLb.indices.filter { timesMs[it] >= start }
    // The running total at the window's opening, so the curve starts where history had got to.
    val opening = seriesLb.indices.lastOrNull { timesMs[it] < start }?.let { seriesLb[it] }
    val points = buildList {
        if (opening != null) add(CurvePoint(0f, opening.toFloat()))
        inside.forEach { add(CurvePoint(((timesMs[it] - start) / span).coerceIn(0f, 1f), seriesLb[it].toFloat())) }
        add(CurvePoint(1f, seriesLb.last().toFloat()))
    }
    val startDate = Instant.ofEpochMilli(start).atZone(zone).toLocalDate()
    val endDate = Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
    var m = YearMonth.from(startDate).plusMonths(1)
    val months = mutableListOf<Pair<Float, String>>()
    val all = mutableListOf<YearMonth>()
    while (!m.isAfter(YearMonth.from(endDate))) { all += m; m = m.plusMonths(1) }
    // Every other month when the window is long, so twelve names never crowd 340dp.
    val step = if (all.size > 7) 2 else 1
    all.forEachIndexed { i, ym ->
        if ((all.size - 1 - i) % step != 0) return@forEachIndexed
        val ms = ym.atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()
        val x = ((ms - start) / span).coerceIn(0f, 1f)
        months += x to ym.month.getDisplayName(java.time.format.TextStyle.SHORT, locale)
    }
    return LifetimeCurve(points, months)
}

// ── Consistency ───────────────────────────────────────────────────────────────────────────────

/** How one day of the month grid is drawn. */
internal enum class DayMark { TRAINED, DELOAD, REST, MISSED, OPEN, TODAY, FUTURE }

internal data class MonthDay(val date: LocalDate, val mark: DayMark, val count: Int, val isToday: Boolean)

/**
 * The month as Monday-first weeks; null cells pad the first and last week. A trained day is lit (a
 * deload session's day faded), a past day the plan called rest or a holiday is [DayMark.REST], a
 * past planned training day with nothing logged is [DayMark.MISSED], and [DayMark.OPEN] is a past
 * day with no fixed plan to judge it by.
 */
internal fun monthGrid(
    month: YearMonth,
    today: LocalDate,
    activityByDay: Map<Long, Int>,
    deloadDays: Set<Long>,
    planOn: (LocalDate) -> Boolean?,
    onVacation: (LocalDate) -> Boolean,
): List<List<MonthDay?>> {
    val first = month.atDay(1)
    val lead = first.dayOfWeek.value - DayOfWeek.MONDAY.value
    val cells = MutableList<MonthDay?>(lead) { null }
    for (d in 1..month.lengthOfMonth()) {
        val date = month.atDay(d)
        val key = date.toEpochDay()
        val count = activityByDay[key] ?: 0
        val mark = when {
            date.isAfter(today) -> DayMark.FUTURE
            count > 0 -> if (key in deloadDays) DayMark.DELOAD else DayMark.TRAINED
            date == today -> DayMark.TODAY
            onVacation(date) -> DayMark.REST
            else -> when (planOn(date)) {
                true -> DayMark.REST
                false -> DayMark.MISSED
                null -> DayMark.OPEN
            }
        }
        cells += MonthDay(date, mark, count, date == today)
    }
    while (cells.size % 7 != 0) cells += null
    return cells.chunked(7)
}

/**
 * Consecutive weeks on plan, ending this week. A week is on plan when you trained at least as many
 * days as it planned (any day counts, so a swapped day is not a miss); with no fixed plan in force,
 * when you trained at all. A week with nothing planned (all holiday or rest) bridges without
 * counting. This week counts once it is on plan so far and never breaks the run while it is still
 * going.
 */
internal fun weeksOnPlan(
    today: LocalDate,
    activityByDay: Map<Long, Int>,
    planOn: (LocalDate) -> Boolean?,
    onVacation: (LocalDate) -> Boolean,
    maxWeeks: Int = 520,
): Int {
    val firstDay = activityByDay.keys.minOrNull() ?: return 0
    // Today is not over: it counts once trained, and is never held against the week before then.
    fun status(start: LocalDate, end: LocalDate): Boolean? {
        var planned = 0
        var anyPlan = false
        var trained = 0
        var open = 0
        var d = start
        while (!d.isAfter(end)) {
            val did = (activityByDay[d.toEpochDay()] ?: 0) > 0
            if (did) trained++
            if (!onVacation(d) && (d != today || did)) {
                when (planOn(d)) {
                    false -> { planned++; anyPlan = true }
                    true -> anyPlan = true
                    null -> open++
                }
            }
            d = d.plusDays(1)
        }
        return when {
            anyPlan -> if (planned == 0) null else trained >= planned
            open == 0 -> null
            else -> trained > 0
        }
    }
    var count = 0
    var weekStart = today.with(TemporalAdjusters.previousOrSame(DayOfWeek.MONDAY))
    if (status(weekStart, today) == true) count++
    repeat(maxWeeks) {
        weekStart = weekStart.minusWeeks(1)
        if (weekStart.plusDays(6).toEpochDay() < firstDay) return count
        when (status(weekStart, weekStart.plusDays(6))) {
            true -> count++
            false -> return count
            null -> Unit
        }
    }
    return count
}

/** "13 of 13 planned days so far · 5 weeks on plan", or "9 days trained" with no fixed plan. */
internal fun consistencyLine(
    month: YearMonth,
    today: LocalDate,
    activityByDay: Map<Long, Int>,
    planOn: (LocalDate) -> Boolean?,
    onVacation: (LocalDate) -> Boolean,
    weeksOnPlan: Int,
): String {
    val current = month == YearMonth.from(today)
    val lastDay = if (current) today.dayOfMonth else month.lengthOfMonth()
    if (month.atDay(1).isAfter(today)) return "Nothing yet"
    val days = (1..lastDay).map { month.atDay(it) }
    val trainedDays = days.filter { (activityByDay[it.toEpochDay()] ?: 0) > 0 }
    val anyPlan = days.any { planOn(it) != null }
    val soFar = if (current) " so far" else ""
    val head = if (anyPlan) {
        // Today counts once it is trained; until then it is not yet a planned day gone by.
        val planned = days.filter {
            !onVacation(it) && planOn(it) == false && (it != today || (activityByDay[it.toEpochDay()] ?: 0) > 0)
        }
        // A day trained in place of a planned one still keeps the plan, so the count is capped by
        // days trained, not by which weekday they fell on.
        val kept = minOf(trainedDays.size, planned.size)
        "$kept of ${planned.size} planned days$soFar"
    } else {
        "${trainedDays.size} ${if (trainedDays.size == 1) "day" else "days"} trained$soFar"
    }
    val run = if (current && weeksOnPlan > 0) " · $weeksOnPlan ${if (weeksOnPlan == 1) "week" else "weeks"} on plan" else ""
    return head + run
}

// ── Body ──────────────────────────────────────────────────────────────────────────────────────

/** A reading drawn as a serif figure with its unit at a smaller size, and its change line. */
internal data class BodyReading(
    val figure: String,
    val unit: String,
    val change: String?,
    /** True when [change] says something moved (ink); false for a quiet line (muted). */
    val moved: Boolean,
    val series: List<Float>,
    val empty: Boolean,
)

private fun avgOf(xs: List<Double>) = xs.sum() / xs.size

/**
 * Weight as its 7-day average: every weigh-in in the week up to the latest one. The change is that
 * average against the 7-day average a month before, or against the first weigh-in when the log is
 * younger than a month.
 */
internal fun weightReading(entries: List<BodyweightEntry>, goalLb: Double?, unit: WeightUnit): BodyReading {
    val sorted = entries.sortedBy { it.recordedAt }
    val last = sorted.lastOrNull()
        ?: return BodyReading("0", " ${unitLabel(unit)}", "Log your first weigh-in", false, emptyList(), true)
    fun avgEnding(t: Long) = sorted.filter { it.recordedAt in (t - 6 * DAY_MS)..t }.map { it.weightLb }.let(::avgOf)
    val avgLb = avgEnding(last.recordedAt)
    val stones = unit == WeightUnit.ST
    val figure = if (stones) formatWeight(avgLb, unit) else String.format(Locale.US, "%.1f", toDisplayWeight(avgLb, unit))
    val ref = sorted.lastOrNull { it.recordedAt <= last.recordedAt - 30 * DAY_MS }
    val (refLb, window) = when {
        ref != null -> avgEnding(ref.recordedAt) to "in 30 days"
        sorted.size >= 2 && sorted.first().recordedAt < last.recordedAt - 6 * DAY_MS ->
            sorted.first().weightLb to "since ${sorted.first().dateKey.let(::shortDate)}"
        else -> null to ""
    }
    val parts = mutableListOf<String>()
    var moved = false
    if (refLb != null) {
        val d = avgLb - refLb
        if (abs(toDisplayWeight(d, unit)) >= 0.05) {
            parts += "${if (d > 0) "↑" else "↓"} ${deltaText(abs(d), unit)} $window"
            moved = true
        } else parts += "Level $window"
    }
    if (goalLb != null) {
        val toGoal = abs(avgLb - goalLb)
        parts += if (toDisplayWeight(toGoal, unit) < 0.5) "at goal" else "${deltaNumber(toGoal, unit)} to goal"
    }
    val series = sorted.filter { it.recordedAt >= last.recordedAt - 90 * DAY_MS }.map { toDisplayWeight(it.weightLb, unit).toFloat() }
    return BodyReading(
        figure,
        if (stones) "" else " ${unitLabel(unit)}",
        parts.joinToString(" · ").ifEmpty { null }?.replaceFirstChar { it.uppercase() },
        moved,
        series,
        false
    )
}

/** A weight difference in the display unit with its unit ("1.4 lb", "0.6 kg", "2 lb" in stones). */
private fun deltaText(lb: Double, unit: WeightUnit): String =
    if (unit == WeightUnit.ST) formatWeightDelta(lb, unit)
    else "${String.format(Locale.US, "%.1f", toDisplayWeight(lb, unit))} ${unitLabel(unit)}"

private fun deltaNumber(lb: Double, unit: WeightUnit): String =
    if (unit == WeightUnit.ST) formatWeightDelta(lb, unit) else String.format(Locale.US, "%.1f", toDisplayWeight(lb, unit))

private fun shortDate(dateKey: String): String =
    runCatching { LocalDate.parse(dateKey).format(java.time.format.DateTimeFormatter.ofPattern("MMM d", Locale.getDefault())) }
        .getOrDefault(dateKey)

/** The goal line on the weight head: "Goal 178", or null with no bodyweight goal. */
internal fun goalLabel(goalLb: Double?, unit: WeightUnit): String? = goalLb?.let {
    if (unit == WeightUnit.ST) "Goal ${formatWeight(it, unit)}" else "Goal ${toDisplayWeight(it, unit).roundToInt()}"
}

/**
 * A composition reading over the last 30 days of its log: the latest value, the change since the
 * first reading inside the window (or across the whole log when the window holds only one), and the
 * readings for the small line.
 */
private fun windowReading(
    times: List<Long>,
    values: List<Double>,
    unit: String,
    decimals: Int,
    deltaUnit: String,
    emptyLine: String,
): BodyReading {
    if (values.isEmpty()) return BodyReading("0", unit, emptyLine, false, emptyList(), true)
    val cutoff = times.last() - 30 * DAY_MS
    val inWindow = values.indices.filter { times[it] >= cutoff }
    val span = if (inWindow.size >= 2) inWindow else values.indices.toList()
    val d = values.last() - values[span.first()]
    val fmt = "%.${decimals}f"
    val change = when {
        span.size < 2 -> "One reading"
        abs(d) < 0.05 -> "Level"
        else -> "${if (d > 0) "↑" else "↓"} ${String.format(Locale.US, "%.1f", abs(d))} $deltaUnit"
    }
    return BodyReading(
        String.format(Locale.US, fmt, values.last()),
        unit,
        change,
        span.size >= 2 && abs(d) >= 0.05,
        span.map { values[it].toFloat() },
        false
    )
}

internal fun bodyFatReading(entries: List<BodyFatEntry>): BodyReading {
    val s = entries.sortedBy { it.recordedAt }
    return windowReading(s.map { it.recordedAt }, s.map { it.percent }, "%", 1, "pts", "Log body fat")
}

internal fun muscleReading(entries: List<LeanMassEntry>, unit: WeightUnit): BodyReading {
    val s = entries.sortedBy { it.recordedAt }
    // Stones would read as a fraction here; muscle mass stays in lb for a stone user, as Stats does.
    val u = if (unit == WeightUnit.ST) WeightUnit.LB else unit
    return windowReading(
        s.map { it.recordedAt }, s.map { toDisplayWeight(it.weightLb, u) },
        unitLabel(u), 1, unitLabel(u), "Sync a watch"
    )
}

/** The featured size: the waist by convention, else whichever site was measured last. */
internal data class SizeReading(val label: String, val reading: BodyReading, val tracked: Int, val total: Int)

internal fun sizeReading(series: List<MeasurementSeries>, useCm: Boolean): SizeReading {
    val tracked = series.filter { it.entries.isNotEmpty() }
    val featured = tracked.firstOrNull { it.type == BodyMeasurementType.WAIST }
        ?: tracked.maxByOrNull { it.entries.last().recordedAt }
    val unit = lengthUnitLabel(useCm)
    val entries = featured?.entries.orEmpty().sortedBy { it.recordedAt }
    return SizeReading(
        label = featured?.type?.label ?: "Waist",
        reading = windowReading(
            entries.map { it.recordedAt }, entries.map { toDisplayLength(it.valueCm, useCm) },
            unit, 1, unit, "Measure"
        ),
        tracked = tracked.size,
        total = BodyMeasurementType.entries.size
    )
}

/**
 * How old a reading is, once it is old enough to matter ("3 WK AGO"), or null while it is recent.
 * The quiet line under a composition reading says this instead of its source when it has gone stale.
 */
internal fun staleReadingAge(recordedAt: Long?, nowMs: Long): String? {
    val days = recordedAt?.let { ((nowMs - it) / DAY_MS).toInt() } ?: return null
    return when {
        days < 14 -> null
        days < 60 -> "${days / 7} WK AGO"
        days < 730 -> "${days / 30} MO AGO"
        else -> "${days / 365} YR AGO"
    }
}

// ── Photos ────────────────────────────────────────────────────────────────────────────────────

/** The first and latest photo of one pose, and how far apart they are. */
internal data class ThenAndNow(
    val then: ProgressPhoto,
    val now: ProgressPhoto,
    val pose: PhotoPose?,
    val apart: String,
    /** now − then in lb, when both photos carry a weight. */
    val weightDeltaLb: Double?,
)

/**
 * The comparison the Photos tab leads with: the Front pose when it has two photos, else the pose
 * with the most, else the first and latest photo of all.
 */
internal fun thenAndNow(photos: List<ProgressPhoto>, zone: ZoneId): ThenAndNow? {
    if (photos.size < 2) return null
    val byPose = photos.groupBy { PhotoPose.fromKey(it.pose) }
    val pose = when {
        (byPose[PhotoPose.FRONT]?.size ?: 0) >= 2 -> PhotoPose.FRONT
        else -> byPose.filterKeys { it != null }.filterValues { it.size >= 2 }.maxByOrNull { it.value.size }?.key
    }
    val set = (if (pose != null) byPose[pose].orEmpty() else photos).sortedBy { it.takenAtMs }
    val then = set.first()
    val now = set.last()
    if (then.takenAtMs == now.takenAtMs && then.fileName == now.fileName) return null
    val a = Instant.ofEpochMilli(then.takenAtMs).atZone(zone).toLocalDate()
    val b = Instant.ofEpochMilli(now.takenAtMs).atZone(zone).toLocalDate()
    val months = ChronoUnit.MONTHS.between(a, b)
    val days = ChronoUnit.DAYS.between(a, b)
    val apart = when {
        months >= 1 -> "$months ${if (months == 1L) "month" else "months"} apart"
        days >= 1 -> "$days ${if (days == 1L) "day" else "days"} apart"
        else -> "Same day"
    }
    val delta = if (then.weightLb != null && now.weightLb != null) now.weightLb - then.weightLb else null
    return ThenAndNow(then, now, pose, apart, delta)
}

/** One month of the gallery: its Front, Side and Back (the latest of each), topped up to three. */
internal data class GalleryMonth(val month: YearMonth, val tiles: List<ProgressPhoto>, val count: Int)

internal fun galleryByMonth(photos: List<ProgressPhoto>, zone: ZoneId): List<GalleryMonth> =
    photos.groupBy { YearMonth.from(Instant.ofEpochMilli(it.takenAtMs).atZone(zone)) }
        .toSortedMap(compareByDescending { it })
        .map { (month, inMonth) ->
            val newest = inMonth.sortedByDescending { it.takenAtMs }
            val posed = listOf(PhotoPose.FRONT, PhotoPose.SIDE, PhotoPose.BACK)
                .mapNotNull { p -> newest.firstOrNull { it.pose == p.name } }
            val rest = newest.filter { it !in posed }
            GalleryMonth(month, (posed + rest).take(3), inMonth.size)
        }
