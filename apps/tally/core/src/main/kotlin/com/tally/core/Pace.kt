package com.tally.core

import kotlin.math.abs
import kotlin.math.roundToLong

/**
 * Where a budget stands against an even spend. The app's one signature reading: "you have spent
 * 62%" says nothing on its own, "$84 over pace with 9 days left" says what to do.
 *
 * [elapsedDays] counts today, so on day 1 of a 31-day month the pace already allows 1/31 of the
 * budget: money spent today is spent today.
 */
data class PaceReading(
    val budget: Long,
    val spent: Long,
    val totalDays: Int,
    val elapsedDays: Int,
) {
    init {
        require(totalDays > 0) { "A period has at least one day" }
    }

    private val elapsed = elapsedDays.coerceIn(0, totalDays)

    /** What an even spend would have used by the end of today. */
    val expected: Long = if (budget <= 0) 0 else (budget.toDouble() * elapsed / totalDays).roundToLong()

    val remaining: Long = budget - spent

    /** Positive when spending runs faster than an even pace. */
    val paceDelta: Long = spent - expected

    /** Days still to spend in, today included. */
    val daysLeft: Int = (totalDays - elapsed + if (elapsed in 1..totalDays) 1 else 0).coerceAtLeast(0)

    /** What can go out each remaining day and still land on budget. Zero once the budget is gone. */
    val dailyAllowance: Long = if (remaining <= 0 || daysLeft == 0) 0 else remaining / daysLeft

    /** Spent as a fraction of the budget; may exceed 1. Zero without a budget. */
    val spentFraction: Float = if (budget <= 0) 0f else (spent.toDouble() / budget).toFloat()

    /** Where the pace tick sits on the meter, 0..1. */
    val paceFraction: Float = elapsed.toFloat() / totalDays

    val status: PaceStatus = when {
        budget <= 0 -> PaceStatus.NO_BUDGET
        spent > budget -> PaceStatus.OVER_BUDGET
        // A tolerance of half a day's allowance, so a single coffee does not flip the verdict.
        abs(paceDelta) <= budget / totalDays / 2 -> PaceStatus.ON_PACE
        paceDelta > 0 -> PaceStatus.OVER_PACE
        else -> PaceStatus.UNDER_PACE
    }
}

enum class PaceStatus { NO_BUDGET, UNDER_PACE, ON_PACE, OVER_PACE, OVER_BUDGET }
