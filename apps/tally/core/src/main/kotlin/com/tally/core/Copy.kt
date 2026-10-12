package com.tally.core

import kotlin.math.abs

/**
 * Every generated sentence the app shows about money. One place, so the voice stays one voice and
 * a test can hold it to the rules: dry, specific, no exclamation marks, no em dashes, no praise the
 * numbers do not support. Lines vary with the numbers rather than repeating one cue.
 */
object Copy {

    fun plural(n: Int, one: String, many: String = one + "s"): String = "$n ${if (n == 1) one else many}"

    /** The margin bar's reading: what each remaining day can take, or how far over it went. */
    fun marginLine(r: PaceReading, fmt: MoneyFormatter): String = when (r.status) {
        PaceStatus.NO_BUDGET -> "No budget set for this month"
        PaceStatus.OVER_BUDGET -> "${fmt.formatWhole(-r.remaining)} over budget, ${plural(r.daysLeft, "day")} left"
        else -> if (r.daysLeft <= 1) {
            "${fmt.formatWhole(r.remaining)} left for today"
        } else {
            "${fmt.formatWhole(r.dailyAllowance)} a day for ${plural(r.daysLeft, "day")}"
        }
    }

    /** The pace verdict, in money rather than percent, so it says what to change. */
    fun paceLine(r: PaceReading, fmt: MoneyFormatter): String = when (r.status) {
        PaceStatus.NO_BUDGET -> ""
        PaceStatus.ON_PACE -> "On pace"
        PaceStatus.OVER_PACE -> "${fmt.formatWhole(r.paceDelta)} over pace"
        PaceStatus.UNDER_PACE -> "${fmt.formatWhole(-r.paceDelta)} under pace"
        PaceStatus.OVER_BUDGET -> "Over budget"
    }

    /**
     * The pace verdict in days: how many days of an even spend the month runs ahead of, or has in
     * hand. Never zero days: a reading off pace is at least one day off it.
     */
    fun daysLine(r: PaceReading): String {
        fun days() = Invest.mulDivHalfEven(abs(r.paceDelta), r.totalDays.toLong(), r.budget).coerceAtLeast(1).toInt()
        return when (r.status) {
            PaceStatus.NO_BUDGET -> ""
            PaceStatus.OVER_BUDGET -> "Over budget"
            PaceStatus.ON_PACE -> "On pace"
            PaceStatus.OVER_PACE -> "${plural(days(), "day")} ahead of your money"
            PaceStatus.UNDER_PACE -> "${plural(days(), "day")} of room in hand"
        }
    }

    /** One envelope's reading, for a budget row: "$212 of $300". */
    fun ofBudget(spent: Long, budget: Long, fmt: MoneyFormatter): String =
        "${fmt.formatWhole(spent)} of ${fmt.formatWhole(budget)}"

    /**
     * The month against the one before, by the same day. Nothing by this day last month says just
     * that: it is not proof of a first month, since earlier months can hold entries on later days.
     */
    fun versusLastLine(thisPeriod: Long, lastPeriodSameDay: Long, fmt: MoneyFormatter): String {
        if (lastPeriodSameDay <= 0L) return if (thisPeriod == 0L) "Nothing spent yet" else "Nothing by this day last month"
        val diff = thisPeriod - lastPeriodSameDay
        return when {
            diff == 0L -> "Level with last month by this day"
            diff > 0 -> "${fmt.formatWhole(diff)} more than last month by this day"
            else -> "${fmt.formatWhole(-diff)} less than last month by this day"
        }
    }

    /** A bill's due line. */
    fun dueLine(daysUntil: Long): String = when {
        daysUntil < 0 -> "Overdue by ${plural((-daysUntil).toInt(), "day")}"
        daysUntil == 0L -> "Due today"
        daysUntil == 1L -> "Due tomorrow"
        else -> "Due in ${plural(daysUntil.toInt(), "day")}"
    }

    /** A goal's pace: what each month needs to land on the date. */
    fun goalLine(saved: Long, target: Long, monthsLeft: Int?, fmt: MoneyFormatter): String {
        val left = target - saved
        return when {
            left <= 0 -> "Reached"
            monthsLeft == null -> "${fmt.formatWhole(left)} to go"
            monthsLeft <= 0 -> "${fmt.formatWhole(left)} to go, date passed"
            else -> "${fmt.formatWhole((left + monthsLeft - 1) / monthsLeft)} a month for ${plural(monthsLeft, "month")}"
        }
    }

    /** Characters and words the app never renders. The doctrine test scans every string with it. */
    val banned: List<String> = listOf("!", "—", "awesome", "crush", "amazing", "great job", "oops")
}
