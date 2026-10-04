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

/**
 * A bill's schedule as the database holds it: the rule it follows, the next date it is due,
 * whether it posts itself (off makes it a reminder) and whether it runs at all.
 */
data class BillSchedule(
    val rule: Recurrence,
    val next: LocalDate,
    val autoPost: Boolean = true,
    val active: Boolean = true,
)

/**
 * The date a bill is next due once it is saved with [rule], [autoPost] and [active] over
 * [stored], the schedule the database holds for it (null for a new bill). The stored schedule is
 * the authority, never an editor's copy of it, so saving cannot post a date twice and never
 * back-fills dates nobody asked for:
 *
 * - A new bill starts at its first date from today, or from [notBefore] when that is later.
 * - A changed rule starts again at its first date from today.
 * - A paused bill keeps its stored date; turning it back on is what moves it.
 * - A bill that was posting itself and still does keeps its stored date even when it is behind:
 *   those dates are owed, and the poster catches them up.
 * - Any other bill (one turned back on, one switched to posting itself, a reminder) keeps its
 *   stored date unless that date is behind, and then starts at its first date from today. A
 *   paused bill does not owe the dates it was paused for, and a reminder's past dates were never
 *   going to post.
 *
 * Every fresh start also falls strictly after [lastPosted], the latest date already entered
 * against the bill, so changing the rule on a day the bill posted does not post that day again.
 */
fun nextDueOnSave(
    rule: Recurrence,
    autoPost: Boolean,
    active: Boolean,
    stored: BillSchedule?,
    today: LocalDate,
    lastPosted: LocalDate? = null,
    notBefore: LocalDate? = null,
): LocalDate {
    val start = if (lastPosted != null && !lastPosted.isBefore(today)) lastPosted.plusDays(1) else today
    if (stored == null) return rule.onOrAfter(if (notBefore != null && notBefore.isAfter(start)) notBefore else start)
    if (stored.rule != rule) return rule.onOrAfter(start)
    if (!active) return stored.next
    if (stored.active && stored.autoPost && autoPost) return stored.next
    return if (stored.next.isBefore(start)) rule.onOrAfter(start) else stored.next
}
