package com.tally.core

import java.time.LocalDate
import java.time.temporal.ChronoUnit

/**
 * One budget period: [start] inclusive to [endExclusive] exclusive. A calendar month by default;
 * with a start day of 15 it runs from the 15th to the 14th of the next month, for people paid on a
 * fixed date. Start days stop at 28 so every month has one.
 */
data class BudgetPeriod(val start: LocalDate, val endExclusive: LocalDate) {

    init {
        require(endExclusive.isAfter(start)) { "A period must be at least one day long" }
    }

    val days: Int get() = ChronoUnit.DAYS.between(start, endExclusive).toInt()

    val lastDay: LocalDate get() = endExclusive.minusDays(1)

    operator fun contains(date: LocalDate): Boolean = !date.isBefore(start) && date.isBefore(endExclusive)

    /** Days elapsed INCLUDING [today]: 1 on the first day, [days] on the last. Clamped to the period. */
    fun elapsedDays(today: LocalDate): Int = when {
        today.isBefore(start) -> 0
        !today.isBefore(endExclusive) -> days
        else -> ChronoUnit.DAYS.between(start, today).toInt() + 1
    }

    /** Days still to spend in, INCLUDING [today]: [days] on the first day, 1 on the last, 0 after. */
    fun daysLeft(today: LocalDate): Int = days - elapsedDays(today) + if (today in this) 1 else 0

    fun dates(): List<LocalDate> = List(days) { start.plusDays(it.toLong()) }

    companion object {
        const val MAX_START_DAY = 28

        /** The period containing [date] for a cycle that starts on [startDay] of each month. */
        fun containing(date: LocalDate, startDay: Int = 1): BudgetPeriod {
            val day = startDay.coerceIn(1, MAX_START_DAY)
            val thisMonthStart = date.withDayOfMonth(day)
            val start = if (date.dayOfMonth >= day) thisMonthStart else thisMonthStart.minusMonths(1)
            return BudgetPeriod(start, start.plusMonths(1))
        }
    }

    /** The period [n] cycles away (negative goes back). */
    fun shift(n: Long): BudgetPeriod {
        val s = start.plusMonths(n)
        return BudgetPeriod(s, s.plusMonths(1))
    }
}
