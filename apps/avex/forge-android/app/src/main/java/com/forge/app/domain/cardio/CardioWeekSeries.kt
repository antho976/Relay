package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.temporal.ChronoUnit

/**
 * One week in the load series — the reading the cardio page could not give before: how this week's
 * volume stands against the ones behind it. Carries the same four numbers a week is judged on, plus
 * the Mon–Sun minutes so a row can draw its own shape without a second pass over the entries.
 */
data class CardioWeekPoint(
    /** Monday 00:00 of this week, epoch ms — the week's identity and its sort key. */
    val weekStartMs: Long,
    val minutes: Int,
    val distanceKm: Double,
    /** Distinct calendar days with an active session. */
    val days: Int,
    val sessions: Int,
    /** Index 0 = Monday … 6 = Sunday; active minutes that day. Always length 7. */
    val perDayMinutes: List<Int>
) {
    /** No active session landed in this week. Its row/bar draws at zero rather than vanishing (§12). */
    val isEmpty: Boolean get() = sessions == 0
}

/**
 * The last [weeks] Mon–Sun weeks ending with the one containing [nowMs], oldest→newest, with no gaps:
 * a week nobody trained in is present at zero, because a load chart that silently drops empty weeks
 * reads as an unbroken streak. Rest entries are excluded throughout (they aren't minutes moved),
 * matching [cardioWeekAggregate].
 *
 * Pure and computed off the in-memory entry list, so both the overview's LOAD bars and the weeks
 * ledger read the same numbers with no extra DB work.
 */
fun cardioWeekSeries(
    entries: List<CardioEntry>,
    nowMs: Long,
    weeks: Int,
    zone: ZoneId = ZoneId.systemDefault()
): List<CardioWeekPoint> {
    if (weeks <= 0) return emptyList()
    val currentMonday = Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate().with(DayOfWeek.MONDAY)
    val firstMonday = currentMonday.minusWeeks((weeks - 1).toLong())

    // One pass over the history, bucketed by week index — an entry outside the window is skipped
    // rather than sorted, so a long history costs a single scan.
    val perDay = Array(weeks) { IntArray(7) }
    val minutes = IntArray(weeks)
    val distance = DoubleArray(weeks)
    val sessions = IntArray(weeks)
    val activeDays = Array(weeks) { HashSet<LocalDate>() }

    for (e in entries) {
        if (e.type == CardioType.REST.code) continue
        val date = Instant.ofEpochMilli(e.date).atZone(zone).toLocalDate()
        val monday = date.with(DayOfWeek.MONDAY)
        val w = ChronoUnit.WEEKS.between(firstMonday, monday).toInt()
        if (w < 0 || w >= weeks) continue
        // Day-of-week from the entry's own date, so a DST week still maps to its seven calendar days.
        val dow = ChronoUnit.DAYS.between(monday, date).toInt().coerceIn(0, 6)
        perDay[w][dow] += e.durationMin
        minutes[w] += e.durationMin
        distance[w] += e.distanceKm ?: 0.0
        sessions[w]++
        activeDays[w] += date
    }

    return (0 until weeks).map { w ->
        CardioWeekPoint(
            weekStartMs = firstMonday.plusWeeks(w.toLong()).atStartOfDay(zone).toInstant().toEpochMilli(),
            minutes = minutes[w],
            distanceKm = distance[w],
            days = activeDays[w].size,
            sessions = sessions[w],
            perDayMinutes = perDay[w].toList()
        )
    }
}

/**
 * How many of the last [weeks] weeks (excluding the one still in progress) cleared [targetMin] —
 * the LOAD section's one caption. Counting the current week would read as a miss every Monday.
 */
fun cardioWeeksOnTarget(series: List<CardioWeekPoint>, targetMin: Int): Int =
    if (series.size < 2 || targetMin <= 0) 0
    else series.dropLast(1).count { it.minutes >= targetMin }

/**
 * This week's minutes against the median of the completed weeks behind it, as a signed percentage —
 * the LOAD reading. Null until there are three completed weeks to compare against (two points is a
 * coin flip, not a baseline) or when that baseline is zero.
 */
fun cardioLoadDeltaPct(series: List<CardioWeekPoint>): Int? {
    val past = series.dropLast(1).map { it.minutes }
    if (past.size < 3) return null
    val sorted = past.sorted()
    val median = if (sorted.size % 2 == 1) sorted[sorted.size / 2].toDouble()
    else (sorted[sorted.size / 2 - 1] + sorted[sorted.size / 2]) / 2.0
    if (median <= 0.0) return null
    val current = series.last().minutes
    return Math.round((current - median) / median * 100).toInt()
}
