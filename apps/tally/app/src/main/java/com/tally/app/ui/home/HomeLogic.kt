package com.tally.app.ui.home

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.tally.app.ui.common.Dates
import java.time.LocalDate

/** How Home sits in the width it is given. */
internal enum class HomeLayout { PHONE, COLUMN, PANES }

/** The phone column below 600dp, one capped column from 600dp, two panes from 840dp. */
internal fun homeLayout(width: Dp): HomeLayout = when {
    width >= PANES_FROM -> HomeLayout.PANES
    width >= COLUMN_FROM -> HomeLayout.COLUMN
    else -> HomeLayout.PHONE
}

private val COLUMN_FROM = 600.dp
private val PANES_FROM = 840.dp

/**
 * The week chart's spoken reading, each day so far by its full name: initials would read
 * Tuesday and Thursday, Saturday and Sunday, as the same letter. When the week opens before the
 * period began, it ends with what those days spent ("$197 of it before 1 October").
 */
internal fun weekDescription(
    weekStart: LocalDate,
    week: List<Long>,
    todayInWeek: Int,
    spentBeforePeriod: Long = 0L,
    periodStart: LocalDate = weekStart,
    format: (Long) -> String,
): String {
    val days = "This week: " + week.take(todayInWeek + 1).mapIndexed { i, v ->
        Dates.weekday(weekStart.plusDays(i.toLong()).dayOfWeek) + " " + format(v)
    }.joinToString(", ")
    if (spentBeforePeriod <= 0L) return days
    return days + ", " + format(spentBeforePeriod) + " of it before " + periodStart.dayOfMonth + " " + Dates.month(periodStart)
}
