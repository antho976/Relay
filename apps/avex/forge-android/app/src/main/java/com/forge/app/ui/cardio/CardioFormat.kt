package com.forge.app.ui.cardio

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.formatPaceSec
import com.forge.app.domain.cardio.paceSecPerUnit
import com.forge.app.domain.cardio.readsAsSpeed
import com.forge.app.domain.cardio.speedPerHour
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.formatDistance
import com.forge.app.domain.units.toDisplayDistance
import java.text.NumberFormat
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToInt

/*
 * The words and numbers the redesigned Cardio screens print (2026-10-21). Plain functions, so the
 * screens stay thin and every line here can be pinned by a unit test without a composition.
 */

/** The speed unit for a distance unit. */
internal fun speedUnitLabel(useMiles: Boolean): String = if (useMiles) "mph" else "km/h"

/**
 * A session's right-hand figure: its pace ("9:31", " /mi"), or its speed for a ride ("14.2", " mph").
 * Null when it logged no distance, so there is nothing to read a pace from.
 */
internal fun sessionFigure(entry: CardioEntry, useMiles: Boolean): Pair<String, String>? {
    if (readsAsSpeed(entry.type)) {
        val s = speedPerHour(entry.durationMin, entry.distanceKm, useMiles) ?: return null
        return String.format(Locale.US, "%.1f", s) to " ${speedUnitLabel(useMiles)}"
    }
    val p = paceSecPerUnit(entry.durationMin, entry.distanceKm, useMiles) ?: return null
    return formatPaceSec(p) to " /${distanceUnitLabel(useMiles)}"
}

/** "30 min · 7.1 mi", or the rest reason on a rest day. */
internal fun sessionSummary(entry: CardioEntry, isRest: Boolean, useMiles: Boolean): String {
    if (isRest) return CardioRestReason.fromCode(entry.restReason)?.displayName ?: "Rest day"
    return buildList {
        add("${entry.durationMin} min")
        entry.distanceKm?.takeIf { it > 0.0 }?.let { add(formatDistance(it, useMiles)) }
    }.joinToString(" · ")
}

/** One decimal of a distance in the display unit, no suffix ("8.8"). */
internal fun distanceNumber(km: Double, useMiles: Boolean): String =
    String.format(Locale.US, "%.1f", toDisplayDistance(km, useMiles))

/**
 * The hero's distance change: "↑ 1.2 vs last" (this week against last week up to the same weekday),
 * "Same as last" when the two round to the same tenth, and moved = the two differ.
 */
internal fun distanceVsLast(thisKm: Double, lastKm: Double, useMiles: Boolean): Pair<String, Boolean> {
    val d = toDisplayDistance(thisKm, useMiles) - toDisplayDistance(lastKm, useMiles)
    val tenth = (abs(d) * 10).roundToInt() / 10.0
    if (tenth == 0.0) return (if (thisKm <= 0.0) "0 last week" else "Same as last") to false
    return "${if (d > 0) "↑" else "↓"} ${String.format(Locale.US, "%.1f", tenth)} vs last" to true
}

/** The meter caption under the hero's day bars. */
internal fun weekMeterCaption(minutes: Int, target: Int, personal: Boolean): String {
    val mark = if (personal) "your weekly target" else "the WHO weekly mark"
    return if (minutes >= target) "$target reached · $mark" else "${target - minutes} min to $target · $mark"
}

/** "Oct 19 to 25", or "Sep 29 to Oct 5" across a month. */
internal fun weekRangeLabel(start: LocalDate, end: LocalDate, locale: Locale): String {
    val md = DateTimeFormatter.ofPattern("MMM d", locale)
    return if (start.month == end.month) "${start.format(md)} to ${end.dayOfMonth}"
    else "${start.format(md)} to ${end.format(md)}"
}

/** "6,210" in the reader's own grouping. */
internal fun formatCount(n: Int, locale: Locale): String = NumberFormat.getIntegerInstance(locale).format(n)

/** An hour of the day as the steps mark labels it: "6a" / "12p", or "06" on a 24-hour clock. */
internal fun hourTick(hour: Int, use24h: Boolean): String = when {
    use24h -> String.format(Locale.US, "%02d", hour)
    hour == 0 -> "12a"
    hour < 12 -> "${hour}a"
    hour == 12 -> "12p"
    else -> "${hour - 12}p"
}

/** A signed pace gap: "0:21" for 21 seconds. */
internal fun paceGap(sec: Int): String = formatPaceSec(abs(sec))

/** "4th" for 4. */
internal fun ordinal(n: Int): String {
    val suffix = if (n % 100 in 11..13) "th" else when (n % 10) { 1 -> "st"; 2 -> "nd"; 3 -> "rd"; else -> "th" }
    return "$n$suffix"
}
