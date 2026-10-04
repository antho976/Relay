package com.tally.core

import java.time.LocalDate
import java.time.temporal.ChronoUnit

enum class Frequency { WEEKLY, MONTHLY, YEARLY }

/**
 * A repeating date: every [interval] [frequency] units from [anchor]. Occurrence k is always
 * computed from the anchor, never chained from the previous one, so a bill anchored on Jan 31
 * lands on Feb 28 and then back on Mar 31 instead of drifting to the 28th forever.
 */
data class Recurrence(val anchor: LocalDate, val frequency: Frequency, val interval: Int = 1) {

    init {
        require(interval in 1..52) { "Interval must be between 1 and 52" }
    }

    fun occurrence(k: Long): LocalDate = when (frequency) {
        Frequency.WEEKLY -> anchor.plusWeeks(k * interval)
        Frequency.MONTHLY -> anchor.plusMonths(k * interval)
        Frequency.YEARLY -> anchor.plusYears(k * interval)
    }

    /** The first occurrence on or after [date]. */
    fun onOrAfter(date: LocalDate): LocalDate {
        if (!date.isAfter(anchor)) return anchor
        val units = when (frequency) {
            Frequency.WEEKLY -> ChronoUnit.WEEKS.between(anchor, date)
            Frequency.MONTHLY -> ChronoUnit.MONTHS.between(anchor, date)
            Frequency.YEARLY -> ChronoUnit.YEARS.between(anchor, date)
        }
        var k = (units / interval).coerceAtLeast(0)
        // The estimate can sit one step early (month clamping); walk forward to the true answer.
        while (occurrence(k).isBefore(date)) k++
        return occurrence(k)
    }

    /** The first occurrence strictly after [date]. */
    fun after(date: LocalDate): LocalDate = onOrAfter(date.plusDays(1))

    /**
     * Every occurrence from [from] through [through], inclusive, capped at [limit] so a bill whose
     * anchor sits years in the past cannot post a thousand rows in one pass.
     */
    fun between(from: LocalDate, through: LocalDate, limit: Int = 366): List<LocalDate> {
        val out = ArrayList<LocalDate>()
        var d = onOrAfter(from)
        while (!d.isAfter(through) && out.size < limit) {
            out += d
            d = after(d)
        }
        return out
    }

    /** Roughly how many occurrences fall in a 30.44-day month, for "per month" readings. */
    fun perMonthFactor(): Double = when (frequency) {
        Frequency.WEEKLY -> 30.44 / (7.0 * interval)
        Frequency.MONTHLY -> 1.0 / interval
        Frequency.YEARLY -> 1.0 / (12.0 * interval)
    }
}
