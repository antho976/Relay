package com.tally.app.ui.insights

import com.tally.app.ui.common.Dates
import java.time.LocalDate

/** An empty day sheet's line: "today" and "yesterday" read as words, any other day by its date. */
internal fun nothingLoggedLine(date: LocalDate, today: LocalDate): String = when (date) {
    today -> "Nothing logged today"
    today.minusDays(1) -> "Nothing logged yesterday"
    else -> "Nothing logged on " + Dates.day(date, today)
}
