package com.tally.app.ui.common

import com.tally.core.BudgetPeriod
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.time.format.TextStyle
import java.util.Locale

/** Human dates. A machine date ("2026-10-04") never renders. */
object Dates {
    private val locale get() = Locale.getDefault()

    private val dayMonth: DateTimeFormatter get() = DateTimeFormatter.ofPattern("d MMM", locale)
    private val weekdayDayMonth: DateTimeFormatter get() = DateTimeFormatter.ofPattern("EEE d MMM", locale)
    private val dayMonthYear: DateTimeFormatter get() = DateTimeFormatter.ofPattern("d MMM yyyy", locale)

    /** "Today", "Yesterday", "Tue 29 Sep", and the year only when it is not this one. */
    fun day(date: LocalDate, today: LocalDate): String = when (date) {
        today -> "Today"
        today.minusDays(1) -> "Yesterday"
        today.plusDays(1) -> "Tomorrow"
        else -> if (date.year == today.year) date.format(weekdayDayMonth) else date.format(dayMonthYear)
    }

    fun short(date: LocalDate, today: LocalDate): String =
        if (date.year == today.year) date.format(dayMonth) else date.format(dayMonthYear)

    fun long(date: LocalDate): String = date.format(DateTimeFormatter.ofLocalizedDate(FormatStyle.LONG).withLocale(locale))

    fun month(date: LocalDate): String = date.month.getDisplayName(TextStyle.FULL_STANDALONE, locale)
        .replaceFirstChar { it.titlecase(locale) }

    fun monthShort(date: LocalDate): String = date.month.getDisplayName(TextStyle.SHORT_STANDALONE, locale)
        .replaceFirstChar { it.titlecase(locale) }.trimEnd('.')

    /**
     * A period's name: "October" for a calendar month, "15 Sep to 14 Oct" for a paid-on-the-15th
     * cycle, with the year when it is not the current one.
     */
    fun period(p: BudgetPeriod, today: LocalDate): String {
        val calendar = p.start.dayOfMonth == 1
        val base = if (calendar) month(p.start) else "${p.start.format(dayMonth)} to ${p.lastDay.format(dayMonth)}"
        return if (p.start.year == today.year || !calendar) base else "$base ${p.start.year}"
    }

    /** One letter, for a drawn axis only: "S" is Saturday and Sunday, so it is never spoken. */
    fun weekdayInitial(dayOfWeek: java.time.DayOfWeek): String =
        dayOfWeek.getDisplayName(TextStyle.NARROW_STANDALONE, locale)

    /** The weekday's full name ("Tuesday"), for a spoken description where initials collide. */
    fun weekday(dayOfWeek: java.time.DayOfWeek): String =
        dayOfWeek.getDisplayName(TextStyle.FULL, locale).replaceFirstChar { it.titlecase(locale) }
}
