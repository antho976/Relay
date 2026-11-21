package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.units.toDisplayDistance
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.temporal.ChronoUnit
import kotlin.math.roundToInt

/*
 * The readings the redesigned Cardio screens draw (2026-10-21) beyond the ones already in this
 * package: the hero's week streak and distance against last week, a session's standing among its own
 * activity, an activity's bests, and the usual steps by this hour. Pure and Android-free, off the
 * in-memory entry list the ViewModels already hold, so none of them costs a query.
 */

/** Cycling reads as a speed (mph, km/h); every other paced activity as a pace per unit. */
fun readsAsSpeed(typeCode: String): Boolean = typeCode == CardioType.CYCLE.code

/** Average speed in display units per hour, or null when distance or duration is missing. */
fun speedPerHour(durationMin: Int, distanceKm: Double?, useMiles: Boolean): Double? {
    if (distanceKm == null || distanceKm <= 0.0 || durationMin <= 0) return null
    return toDisplayDistance(distanceKm, useMiles) / (durationMin / 60.0)
}

private fun CardioEntry.localDate(zone: ZoneId): LocalDate = Instant.ofEpochMilli(date).atZone(zone).toLocalDate()

private val CardioEntry.isActive: Boolean get() = type != CardioType.REST.code

/**
 * Consecutive Mon–Sun weeks with at least one active session, ending with this week when it already
 * has one, else with last week (a Monday with nothing logged yet has not broken anything).
 */
fun cardioWeekStreak(entries: List<CardioEntry>, zone: ZoneId, today: LocalDate): Int {
    val weeks = entries.asSequence().filter { it.isActive }
        .mapTo(HashSet()) { it.localDate(zone).with(DayOfWeek.MONDAY) }
    if (weeks.isEmpty()) return 0
    val thisMonday = today.with(DayOfWeek.MONDAY)
    var cursor = when {
        thisMonday in weeks -> thisMonday
        thisMonday.minusWeeks(1) in weeks -> thisMonday.minusWeeks(1)
        else -> return 0
    }
    var n = 0
    while (cursor in weeks) { n++; cursor = cursor.minusWeeks(1) }
    return n
}

/**
 * Distance (km) logged LAST week up to and including the same weekday as [today], so a Wednesday
 * compares against last Monday-to-Wednesday rather than against a whole week it has not had yet.
 */
fun lastWeekToDateKm(entries: List<CardioEntry>, zone: ZoneId, today: LocalDate): Double {
    val lastMonday = today.with(DayOfWeek.MONDAY).minusWeeks(1)
    val lastSameDay = today.minusWeeks(1)
    return entries.asSequence()
        .filter { it.isActive }
        .filter { val d = it.localDate(zone); !d.isBefore(lastMonday) && !d.isAfter(lastSameDay) }
        .sumOf { it.distanceKm ?: 0.0 }
}

/** The most recently logged distinct activity codes, newest first, rest days left out. */
fun recentCardioTypes(entries: List<CardioEntry>, limit: Int = 3): List<String> =
    entries.asSequence()
        .filter { it.isActive }
        .sortedByDescending { it.date }
        .map { it.type }
        .distinct()
        .take(limit)
        .toList()

/**
 * The usual step count by the end of [hour] (0..23): the mean, over the past days that have any
 * steps at all, of each day's steps up to that hour. Null until three such days exist, since one or
 * two days are an anecdote rather than a usual.
 */
fun usualStepsByHour(days: List<CardioWearableDay>, hour: Int): Int? {
    val withData = days.filter { d -> d.hourlySteps.any { it.steps > 0 } }
    if (withData.size < 3) return null
    return withData.map { d -> d.hourlySteps.filter { it.hour <= hour }.sumOf { it.steps } }.average().roundToInt()
}

/** Where one session sits among every session of its activity that has the same reading. */
data class CardioStanding(
    /** Every session's value (this one's included), in the reading's own unit. */
    val values: List<Double>,
    val mine: Double,
    /** 1 = the best of them (longest, or fastest). */
    val rank: Int,
)

/** This session's distance among every same-activity session that logged one. */
fun distanceStanding(entry: CardioEntry, all: List<CardioEntry>): CardioStanding? {
    val mine = entry.distanceKm?.takeIf { it > 0.0 } ?: return null
    val pool = all.filter { it.type == entry.type && it.id != entry.id }.mapNotNull { it.distanceKm?.takeIf { d -> d > 0.0 } }
    if (pool.isEmpty()) return null
    return CardioStanding(values = pool + mine, mine = mine, rank = 1 + pool.count { it > mine })
}

/** This session's pace (sec/km) among every same-activity session with a pace; rank 1 is fastest. */
fun paceStanding(entry: CardioEntry, all: List<CardioEntry>): CardioStanding? {
    val mine = paceSecPerKm(entry.durationMin, entry.distanceKm)?.toDouble() ?: return null
    val pool = all.filter { it.type == entry.type && it.id != entry.id }
        .mapNotNull { paceSecPerKm(it.durationMin, it.distanceKm)?.toDouble() }
    if (pool.isEmpty()) return null
    return CardioStanding(values = pool + mine, mine = mine, rank = 1 + pool.count { it < mine })
}

/** The median duration of the same activity's OTHER sessions, or null with fewer than two of them. */
fun usualDurationMin(entry: CardioEntry, all: List<CardioEntry>): Int? =
    all.filter { it.type == entry.type && it.id != entry.id && it.durationMin > 0 }.map { it.durationMin }.median()

/** The median distance (km) of the same activity's OTHER sessions that logged one. */
fun usualDistanceKm(entry: CardioEntry, all: List<CardioEntry>): Double? {
    val d = all.filter { it.type == entry.type && it.id != entry.id }.mapNotNull { it.distanceKm?.takeIf { v -> v > 0.0 } }.sorted()
    if (d.size < 2) return null
    return if (d.size % 2 == 1) d[d.size / 2] else (d[d.size / 2 - 1] + d[d.size / 2]) / 2.0
}

private fun List<Int>.median(): Int? {
    if (size < 2) return null
    val s = sorted()
    return if (s.size % 2 == 1) s[s.size / 2] else ((s[s.size / 2 - 1] + s[s.size / 2]) / 2.0).roundToInt()
}

/**
 * How many weeks this session's distance has been unbeaten for: the weeks back to the most recent
 * EARLIER same-activity session at least as long. [Int.MAX_VALUE] when nothing earlier ever was
 * (the longest yet), null when this session is not the longest of even the last two weeks.
 */
fun longestInWeeks(entry: CardioEntry, all: List<CardioEntry>, zone: ZoneId): Int? {
    val mine = entry.distanceKm?.takeIf { it > 0.0 } ?: return null
    val earlier = all.filter { it.type == entry.type && it.id != entry.id && it.date < entry.date }
    if (earlier.none { (it.distanceKm ?: 0.0) > 0.0 }) return null
    val beaten = earlier.filter { (it.distanceKm ?: 0.0) >= mine }.maxByOrNull { it.date }
        ?: return Int.MAX_VALUE
    val weeks = ChronoUnit.WEEKS.between(beaten.localDate(zone), entry.localDate(zone)).toInt()
    return weeks.takeIf { it >= 2 }
}

/** An activity's bests: the fastest session, the longest one, and the biggest week by distance. */
data class CardioBests(
    val fastest: CardioEntry?,
    val longest: CardioEntry?,
    /** km in the activity's biggest Mon–Sun week, and that week's Monday (epoch ms). */
    val bestWeekKm: Double,
    val bestWeekStartMs: Long?,
)

/** Bests for one activity code, or null when it has no session with a distance. */
fun cardioBests(entries: List<CardioEntry>, typeCode: String, zone: ZoneId): CardioBests? {
    val paced = entries.filter { it.type == typeCode && (it.distanceKm ?: 0.0) > 0.0 && it.durationMin > 0 }
    if (paced.isEmpty()) return null
    val fastest = paced.minByOrNull { paceSecPerKm(it.durationMin, it.distanceKm) ?: Int.MAX_VALUE }
    val longest = paced.maxByOrNull { it.distanceKm ?: 0.0 }
    val byWeek = paced.groupBy { it.localDate(zone).with(DayOfWeek.MONDAY) }
        .mapValues { (_, list) -> list.sumOf { it.distanceKm ?: 0.0 } }
    val best = byWeek.maxByOrNull { it.value }
    return CardioBests(
        fastest = fastest,
        longest = longest,
        bestWeekKm = best?.value ?: 0.0,
        bestWeekStartMs = best?.key?.atStartOfDay(zone)?.toInstant()?.toEpochMilli(),
    )
}

/**
 * One activity's pace (or speed) over the last [weeks] weeks: each paced session as (date, value),
 * where value is seconds per unit for a pace and units per hour for a speed. Falls back to the whole
 * history when the window holds fewer than two sessions, and says so through [allTime].
 */
data class CardioPaceWindow(
    val points: List<Pair<Long, Double>>,
    val startMs: Long,
    val endMs: Long,
    val allTime: Boolean,
    val speed: Boolean,
)

fun cardioPaceWindow(series: CardioPaceSeries, todayStartMs: Long, useMiles: Boolean, weeks: Int = 12): CardioPaceWindow? {
    val speed = readsAsSpeed(series.typeCode)
    val all = series.points.mapNotNull { p ->
        val v = if (speed) speedPerHour(p.durationMin, p.distanceKm, useMiles)
        else paceSecPerUnit(p.durationMin, p.distanceKm, useMiles)?.toDouble()
        v?.let { p.dateMs to it }
    }
    if (all.size < 2) return null
    val endMs = todayStartMs + DAY_MS
    val startMs = endMs - weeks * 7L * DAY_MS
    val inWindow = all.filter { it.first >= startMs }
    return if (inWindow.size >= 2) {
        CardioPaceWindow(inWindow, startMs, endMs, allTime = false, speed = speed)
    } else {
        CardioPaceWindow(all, all.first().first, maxOf(endMs, all.last().first), allTime = true, speed = speed)
    }
}

private const val DAY_MS = 24L * 60 * 60 * 1000
