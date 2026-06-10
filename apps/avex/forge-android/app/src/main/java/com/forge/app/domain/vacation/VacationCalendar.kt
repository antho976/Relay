package com.forge.app.domain.vacation

import com.forge.app.data.db.entities.VacationPeriod
import java.time.LocalDate

/**
 * Pure helper for "is this calendar day inside a holiday/vacation window?" (#135).
 * Vacation days don't break a training streak and aren't counted as missed — the streak
 * computations bridge across them. Date keys are "yyyy-MM-dd"; an unparseable key is
 * treated as not-covering rather than crashing the streak math.
 */
object VacationCalendar {

    /** Returns a predicate over [LocalDate] that is true on any day within a vacation range (inclusive). */
    fun onVacation(periods: List<VacationPeriod>): (LocalDate) -> Boolean {
        val ranges = periods.mapNotNull { p ->
            val start = runCatching { LocalDate.parse(p.startDate) }.getOrNull() ?: return@mapNotNull null
            val end = runCatching { LocalDate.parse(p.endDate) }.getOrNull() ?: return@mapNotNull null
            if (end.isBefore(start)) end to start else start to end
        }
        return { date -> ranges.any { (s, e) -> !date.isBefore(s) && !date.isAfter(e) } }
    }
}
