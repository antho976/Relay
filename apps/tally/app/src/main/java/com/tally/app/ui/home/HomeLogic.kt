package com.tally.app.ui.home

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.CategoryTotal
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.LineTone
import com.tally.app.ui.common.ToneLine
import com.tally.app.ui.plan.limitReached
import com.tally.core.Copy
import com.tally.core.MoneyFormatter
import com.tally.core.PaceReading
import com.tally.core.PaceStatus
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

/** One category's part of the month's spending, for Overview's Where it went. */
@androidx.compose.runtime.Immutable
data class CategoryShare(val categoryId: Long?, val name: String, val icon: String?, val color: Int?, val total: Long, val count: Int)

/**
 * The month's spending by category, largest first, named from [categories]; spending without a
 * category reads "Uncategorized". [limit] keeps Overview to the few that matter.
 */
internal fun topCategories(totals: List<CategoryTotal>, categories: List<CategoryEntity>, limit: Int = 4): List<CategoryShare> {
    val byId = categories.associateBy { it.id }
    return totals.filter { it.total > 0 }.sortedByDescending { it.total }.take(limit).map { t ->
        val cat = t.categoryId?.let { byId[it] }
        CategoryShare(t.categoryId, cat?.name ?: "Uncategorized", cat?.icon, cat?.color, t.total, t.count)
    }
}

/**
 * The hero's line under the figure: the pace in days while there is room, else how far past the
 * limit the month went, as a positive amount.
 */
internal fun heroLine(r: PaceReading, fmt: MoneyFormatter): ToneLine = when {
    !limitReached(r) -> ToneLine(Copy.daysLine(r), if (r.status == PaceStatus.OVER_PACE) LineTone.WARN else LineTone.QUIET)
    r.remaining == 0L -> ToneLine("All of your " + fmt.formatWhole(r.budget) + " is spent", LineTone.HELD)
    else -> ToneLine(fmt.formatWhole(-r.remaining) + " past your " + fmt.formatWhole(r.budget) + " limit", LineTone.HELD)
}

/**
 * Today against its share: the room the month had when today began, split over the days left
 * (today included). Without a budget it is only what today took.
 */
internal fun todayLine(spentToday: Long, r: PaceReading?, fmt: MoneyFormatter): ToneLine {
    if (r == null || r.budget <= 0) return ToneLine(if (spentToday == 0L) "Nothing spent yet today" else "Spent so far today")
    // The hero already says the limit is reached, in red; today only says what it took.
    if (limitReached(r)) return ToneLine(if (spentToday == 0L) "Nothing spent yet today" else "Spent so far today")
    val room = r.budget - (r.spent - spentToday)
    val share = if (r.daysLeft > 0) room / r.daysLeft else room
    return when {
        spentToday == 0L -> ToneLine(fmt.formatWhole(share) + " for today")
        spentToday <= share -> ToneLine(fmt.formatWhole(share - spentToday) + " left for today")
        else -> ToneLine(fmt.formatWhole(spentToday - share) + " past today's share", LineTone.WARN)
    }
}

/** The status bar's month: what is left and the days, or that the limit is reached; never a negative. */
internal fun statusMonthLine(left: Long?, spent: Long, daysLeft: Int, fmt: MoneyFormatter): ToneLine {
    val days = Copy.plural(daysLeft, "day")
    return when {
        left == null -> ToneLine(fmt.formatWhole(spent) + " spent · " + days)
        left > 0 -> ToneLine(fmt.formatWhole(left) + " left · " + days)
        else -> ToneLine("Limit reached · $days", LineTone.HELD)
    }
}
