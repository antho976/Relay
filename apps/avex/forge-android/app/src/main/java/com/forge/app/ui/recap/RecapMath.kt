package com.forge.app.ui.recap

import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.durationMinutes
import com.forge.app.domain.adapt.E1rm
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeightDelta
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.trimDecimal
import com.forge.app.domain.units.unitLabel
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale
import kotlin.math.roundToInt

/*
 * The Recap page's readings (2026-10-21 redesign, "pg-recap"): the month so far against the month
 * before, week by week, then what stood out; the year as every month, the best one lit. Pure, so
 * each figure and comparison is unit-tested.
 */

/** One of the four figures: [value] serif with [suffix] small and muted, and its mono change. */
data class RecapFigure(
    val label: String,
    val value: String,
    val suffix: String = "",
    val change: String,
    val changeMoved: Boolean,
)

/** A highlight: a mono label, the finding, and the fact that backs it. */
data class RecapHighlight(val label: String, val value: String, val sub: String)

data class MonthRecapUi(
    val month: YearMonth,
    val name: String,
    val previousName: String,
    val daysLine: String,
    val figures: List<RecapFigure>,
    /** Volume per week (lb), Wk 1 to Wk 4 (the fourth runs to the month's end). */
    val weeks: List<Double>,
    val previousWeeks: List<Double>,
    /** The week still going, when the month is the current one. */
    val currentWeek: Int?,
    val weekCaption: String?,
    val highlights: List<RecapHighlight>,
    val empty: Boolean,
)

data class YearRecapUi(
    val year: Int,
    val sinceLine: String,
    val figures: List<RecapFigure>,
    /** Workouts per month, January first, twelve of them. */
    val months: List<Int>,
    val bestMonth: Int?,
    /** The month still going, when the year is the current one (0-based). */
    val currentMonth: Int?,
    val bestLine: String?,
    val caption: String?,
    val highlights: List<RecapHighlight>,
    val empty: Boolean,
)

/** A set as the recap needs it: which lift, when, and its estimated 1RM. */
internal data class RecapSet(val exerciseId: String, val swappedName: String?, val atMs: Long, val weightLb: Double, val reps: Int) {
    val e1rm: Double get() = E1rm.epley(weightLb, reps)
}

private fun Session.date(zone: ZoneId): LocalDate = Instant.ofEpochMilli(startedAt).atZone(zone).toLocalDate()

/** "↑ 2 on Sep", "↓ 1 on Sep", "Same as Sep". */
internal fun countChange(now: Int, before: Int, beforeName: String): Pair<String, Boolean> {
    val d = now - before
    return when {
        d > 0 -> "↑ $d on $beforeName" to true
        d < 0 -> "↓ ${-d} on $beforeName" to true
        else -> "Same as $beforeName" to false
    }
}

/** "↑ 6%" against [before], or null when there was nothing to compare with. */
internal fun percentChange(now: Double, before: Double): String? {
    if (before <= 0.0) return null
    val p = ((now - before) / before * 100).roundToInt()
    return when {
        p > 0 -> "↑ $p%"
        p < 0 -> "↓ ${-p}%"
        else -> "Level"
    }
}

/** A volume as a serif figure and its small suffix: (248, "k"), (2.1, "M"), (950, ""). */
internal fun volumeParts(lb: Double, unit: WeightUnit): Pair<String, String> {
    val v = toDisplayWeight(lb, unit)
    return when {
        v >= 1_000_000 -> trimDecimal(v / 1_000_000) to "M"
        v >= 10_000 -> "${(v / 1000).roundToInt()}" to "k"
        v >= 1000 -> trimDecimal(v / 1000) to "k"
        else -> "${v.roundToInt()}" to ""
    }
}

/** Which of the four week buckets a day falls in: 1-7, 8-14, 15-21, 22 to the end. */
internal fun weekBucket(dayOfMonth: Int): Int = ((dayOfMonth - 1) / 7).coerceAtMost(3)

/** Volume per week bucket of a month. */
internal fun weeklyVolume(sessions: List<Session>, zone: ZoneId): List<Double> {
    val out = DoubleArray(4)
    sessions.forEach { s -> out[weekBucket(s.date(zone).dayOfMonth)] += s.totalVolumeLb ?: 0.0 }
    return out.toList()
}

/**
 * The lift that gained most on its best estimated 1RM inside [fromMs, toMs) over everything before
 * [fromMs]: its id, the gain in lb, and the set that did it. Null when nothing beat an earlier best.
 */
internal fun biggestJump(
    sets: List<RecapSet>,
    fromMs: Long,
    toMs: Long,
    /** With no history before the period, measure from the lift's first session inside it ("since Jan"). */
    fromFirstInPeriod: Boolean = false,
): Triple<RecapSet, Double, Double>? {
    val byLift = sets.filter { it.weightLb > 0 }.groupBy { it.exerciseId }
    return byLift.mapNotNull { (_, rows) ->
        val inside = rows.filter { it.atMs in fromMs until toMs }
        val firstDay = inside.minOfOrNull { it.atMs }?.let { f -> inside.filter { it.atMs < f + 86_400_000L } }
        val before = rows.filter { it.atMs < fromMs }.maxOfOrNull { it.e1rm }
            ?: (if (fromFirstInPeriod) firstDay?.maxOfOrNull { it.e1rm } else null)
            ?: return@mapNotNull null
        val best = rows.filter { it.atMs in fromMs until toMs }.maxByOrNull { it.e1rm } ?: return@mapNotNull null
        val gain = best.e1rm - before
        if (gain <= 0.5) null else Triple(best, gain, before)
    }.maxByOrNull { it.second }
}

/** A lift's short name for a headline: "Bench press" stays, "Barbell Bench Press" → "Bench Press". */
private fun shortLift(name: String): String {
    val words = name.split(' ').filter { it.isNotBlank() }
    val drop = setOf("barbell", "dumbbell", "cable", "machine", "smith")
    val kept = words.dropWhile { it.lowercase() in drop }
    return kept.ifEmpty { words }.joinToString(" ")
}

internal fun buildMonthRecap(
    month: YearMonth,
    sessions: List<Session>,
    previous: List<Session>,
    sets: List<RecapSet>,
    topExercise: Pair<String, Int>?,
    today: LocalDate,
    unit: WeightUnit,
    locale: Locale,
    zone: ZoneId,
    liftName: (exerciseId: String, swappedName: String?) -> String,
    dayName: (dayKey: String) -> String?,
): MonthRecapUi {
    val name = month.month.getDisplayName(TextStyle.FULL_STANDALONE, locale)
    val prevName = month.minusMonths(1).month.getDisplayName(TextStyle.SHORT_STANDALONE, locale)
    val current = YearMonth.from(today) == month
    val daysLine = if (current) "${today.dayOfMonth} of ${month.lengthOfMonth()} days" else "${month.lengthOfMonth()} days"
    val (wc, wm) = countChange(sessions.size, previous.size, prevName)
    val vol = sessions.sumOf { it.totalVolumeLb ?: 0.0 }
    val prevVol = previous.sumOf { it.totalVolumeLb ?: 0.0 }
    val (vv, vs) = volumeParts(vol, unit)
    val volPct = percentChange(vol, prevVol)
    val prs = sessions.sumOf { it.prCount }
    val (pc, pm) = countChange(prs, previous.sumOf { it.prCount }, prevName)
    val setsN = sessions.sumOf { it.setCount }
    val setsD = setsN - previous.sumOf { it.setCount }
    val figures = listOf(
        RecapFigure("Workouts", "${sessions.size}", change = wc, changeMoved = wm),
        RecapFigure("Volume", vv, vs, change = listOfNotNull(unitLabel(unit), volPct).joinToString(" · "), changeMoved = volPct != null && volPct != "Level"),
        RecapFigure("PRs", "$prs", change = pc, changeMoved = pm),
        RecapFigure(
            "Sets", "$setsN",
            change = when { setsD > 0 -> "↑ $setsD"; setsD < 0 -> "↓ ${-setsD}"; else -> "Same" },
            changeMoved = setsD != 0
        ),
    )
    val currentWeek = if (current) weekBucket(today.dayOfMonth) else null
    val deloadWeeks = sessions.filter { it.sessionType == "deload" }.map { weekBucket(it.date(zone).dayOfMonth) }.distinct().sorted()
    val weekCaption = buildList {
        deloadWeeks.forEach { add("Week ${it + 1} was a deload") }
        currentWeek?.let { add("Week ${it + 1} is still going") }
    }.joinToString(" · ").ifEmpty { null }

    val from = month.atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()
    val to = month.plusMonths(1).atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()
    val highlights = if (sessions.isEmpty()) emptyList() else buildList {
        topExercise?.let { (n, count) -> add(RecapHighlight("Most trained", n, "$count ${if (count == 1) "session" else "sessions"}")) }
        biggestJump(sets, from, to)?.let { (set, gain, _) ->
            add(
                RecapHighlight(
                    "Biggest jump",
                    "${shortLift(liftName(set.exerciseId, set.swappedName))} +${formatWeightDelta(gain.roundToInt().toDouble(), unit)}",
                    "${trimDecimal(toDisplayWeight(set.weightLb, unit))} × ${set.reps} on ${DateTimeFormatter.ofPattern("MMM d", locale).format(Instant.ofEpochMilli(set.atMs).atZone(zone))}"
                )
            )
        }
        val mins = sessions.mapNotNull { it.durationMinutes() }
        if (mins.isNotEmpty()) {
            val avg = mins.average().roundToInt()
            val prevMins = previous.mapNotNull { it.durationMinutes() }
            val sub = if (prevMins.isEmpty()) "Across ${mins.size} ${if (mins.size == 1) "session" else "sessions"}"
            else countChange(avg, prevMins.average().roundToInt(), prevName).first
            add(RecapHighlight("Avg session", "$avg min", sub))
        }
        val best = sessions.filter { it.prCount > 0 }.maxWithOrNull(compareBy<Session> { it.prCount }.thenBy { it.totalVolumeLb ?: 0.0 })
            ?: sessions.maxByOrNull { it.totalVolumeLb ?: 0.0 }
        best?.let { s ->
            val label = DateTimeFormatter.ofPattern("EEE, MMM d", locale).format(s.date(zone))
            val day = dayName(s.dayKey)
            val why = if (s.prCount > 0) "${s.prCount} ${if (s.prCount == 1) "PR" else "PRs"}" else "Most volume"
            add(RecapHighlight("Best day", label, listOfNotNull(day, why).joinToString(" · ")))
        }
    }
    return MonthRecapUi(
        month = month,
        name = name,
        previousName = month.minusMonths(1).month.getDisplayName(TextStyle.FULL_STANDALONE, locale),
        daysLine = daysLine,
        figures = figures,
        weeks = weeklyVolume(sessions, zone),
        previousWeeks = weeklyVolume(previous, zone),
        currentWeek = currentWeek,
        weekCaption = weekCaption,
        highlights = highlights,
        empty = sessions.isEmpty(),
    )
}

internal fun buildYearRecap(
    year: Int,
    sessions: List<Session>,
    sets: List<RecapSet>,
    topExercise: Pair<String, Int>?,
    longestStreakDays: Int,
    today: LocalDate,
    unit: WeightUnit,
    locale: Locale,
    zone: ZoneId,
    liftName: (exerciseId: String, swappedName: String?) -> String,
): YearRecapUi {
    val current = today.year == year
    val end = if (current) today else LocalDate.of(year, 12, 31)
    val first = sessions.minOfOrNull { it.date(zone) }
    val from = maxOf(LocalDate.of(year, 1, 1), first ?: LocalDate.of(year, 1, 1))
    val weeks = (java.time.temporal.ChronoUnit.DAYS.between(from, end) / 7.0).coerceAtLeast(1.0)
    val perWeek = { n: Int ->
        val r = n / weeks
        when {
            n == 0 -> "None yet"
            r >= 1 -> "~${r.roundToInt()} a week"
            else -> "~${(r * 4.345).roundToInt().coerceAtLeast(1)} a month"
        }
    }
    val (vv, vs) = volumeParts(sessions.sumOf { it.totalVolumeLb ?: 0.0 }, unit)
    val prs = sessions.sumOf { it.prCount }
    val months = IntArray(12)
    sessions.forEach { months[it.date(zone).monthValue - 1]++ }
    val bestMonth = months.withIndex().filter { it.value > 0 }.maxByOrNull { it.value }?.index
    val monthName = { i: Int -> java.time.Month.of(i + 1).getDisplayName(TextStyle.FULL_STANDALONE, locale) }
    val figures = listOf(
        RecapFigure("Workouts", "${sessions.size}", change = perWeek(sessions.size), changeMoved = false),
        RecapFigure("Volume", vv, vs, change = unitLabel(unit), changeMoved = false),
        RecapFigure("PRs", "$prs", change = perWeek(prs), changeMoved = false),
        RecapFigure("Best run", "$longestStreakDays", " d", change = "In a row", changeMoved = false),
    )
    val yFrom = LocalDate.of(year, 1, 1).atStartOfDay(zone).toInstant().toEpochMilli()
    val yTo = LocalDate.of(year + 1, 1, 1).atStartOfDay(zone).toInstant().toEpochMilli()
    val highlights = if (sessions.isEmpty()) emptyList() else buildList {
        topExercise?.let { (n, count) -> add(RecapHighlight("Most trained", n, "$count ${if (count == 1) "session" else "sessions"}")) }
        biggestJump(sets, yFrom, yTo, fromFirstInPeriod = true)?.let { (set, gain, _) ->
            add(RecapHighlight("Biggest gain", "${shortLift(liftName(set.exerciseId, set.swappedName))} +${formatWeightDelta(gain.roundToInt().toDouble(), unit)}", "Est. 1RM, since Jan"))
        }
        bestMonth?.let { add(RecapHighlight("Best month", monthName(it), "${months[it]} ${if (months[it] == 1) "workout" else "workouts"}")) }
        sessions.groupBy { it.date(zone).dayOfWeek }.maxByOrNull { it.value.size }?.let { (d, list) ->
            add(RecapHighlight("Favourite day", dayPlural(d, locale), "${list.size} ${if (list.size == 1) "workout" else "workouts"}"))
        }
    }
    return YearRecapUi(
        year = year,
        sinceLine = if (current) "So far · since Jan 1" else "Jan to Dec",
        figures = figures,
        months = months.toList(),
        bestMonth = bestMonth,
        currentMonth = if (current) today.monthValue - 1 else null,
        bestLine = bestMonth?.let { "Best: ${java.time.Month.of(it + 1).getDisplayName(TextStyle.SHORT_STANDALONE, locale)}" },
        caption = if (current) "${monthName(today.monthValue - 1)} is still going" else null,
        highlights = highlights,
        empty = sessions.isEmpty(),
    )
}

/** "Mondays". English plural on an English name; other languages keep the day's own name. */
private fun dayPlural(d: DayOfWeek, locale: Locale): String {
    val n = d.getDisplayName(TextStyle.FULL_STANDALONE, locale)
    return if (locale.language == "en") "${n}s" else n
}

