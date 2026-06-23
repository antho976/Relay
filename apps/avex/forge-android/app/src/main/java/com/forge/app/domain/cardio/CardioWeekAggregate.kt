package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.temporal.ChronoUnit

/**
 * One week's cardio rolled up for the stats page — active days/sessions/minutes/distance, the Mon–Sun
 * per-day minutes (drives the bar graph), and minutes split by activity type (drives the breakdown
 * graph). Rest entries are excluded throughout (they aren't "minutes moved"). Pure + unit-tested so
 * the swipeable week pager can compute any week off the in-memory entry list with no extra DB reads.
 */
data class CardioWeekAggregate(
    val days: Int,
    val sessions: Int,
    val minutes: Int,
    val distanceKm: Double,
    /** Index 0 = Monday … 6 = Sunday; active minutes that day. Always length 7. */
    val perDayMinutes: List<Int>,
    /** Active minutes by type, busiest first. */
    val minutesByType: List<Pair<CardioType, Int>>
)

/**
 * Roll up the active entries that fall in the Mon–Sun week beginning at [weekStartMs] (a Monday
 * start-of-day millis). Day bucketing uses [LocalDate] (not fixed 7×24h) so a DST week still maps
 * cleanly to its seven calendar days.
 */
fun cardioWeekAggregate(entries: List<CardioEntry>, weekStartMs: Long, zone: ZoneId): CardioWeekAggregate {
    val weekStart = Instant.ofEpochMilli(weekStartMs).atZone(zone).toLocalDate()
    val perDay = IntArray(7)
    val byType = LinkedHashMap<CardioType, Int>()
    val activeDays = HashSet<LocalDate>()
    var sessions = 0
    var minutes = 0
    var distance = 0.0
    for (e in entries) {
        if (e.type == CardioType.REST.code) continue
        val date = Instant.ofEpochMilli(e.date).atZone(zone).toLocalDate()
        val idx = ChronoUnit.DAYS.between(weekStart, date)
        if (idx < 0 || idx > 6) continue
        perDay[idx.toInt()] += e.durationMin
        minutes += e.durationMin
        distance += e.distanceKm ?: 0.0
        sessions++
        activeDays += date
        val t = CardioType.fromCode(e.type)
        byType[t] = (byType[t] ?: 0) + e.durationMin
    }
    return CardioWeekAggregate(
        days = activeDays.size,
        sessions = sessions,
        minutes = minutes,
        distanceKm = distance,
        perDayMinutes = perDay.toList(),
        minutesByType = byType.entries.sortedByDescending { it.value }.map { it.key to it.value }
    )
}
