package com.tally.app.ui.plan

import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.core.BudgetPeriod
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import java.time.LocalDate
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The reads every goal reading needs, for Plan, Home and the goal screen alike: the goals, the
 * accounts, and the month running now with the [MONTHS] before it (a monthly goal's record).
 */
@Singleton
class GoalSource @Inject constructor(
    private val plan: PlanRepository,
    private val ledger: LedgerRepository,
) {
    /** The inputs for [period], the month running on [today]. */
    fun inputs(period: BudgetPeriod, today: LocalDate): Flow<GoalInputs> {
        val periods = (0..MONTHS).map { period.shift(-it.toLong()) }
        val totals = combine(periods.map { p -> ledger.typeTotals(p.start, p.endExclusive) }) { it.toList() }
        return combine(totals, ledger.balances(), ledger.transfers(periods.last().start, period.endExclusive)) { t, b, tr ->
            GoalInputs(today = today, periods = periods, totals = t, balances = b, transfers = tr)
        }
    }

    /** Every goal that is not archived, read. */
    fun reading(period: BudgetPeriod, today: LocalDate): Flow<GoalsReading> =
        combine(plan.goals(), inputs(period, today)) { rows, i -> goalsReading(rows, i) }

    companion object {
        /** A monthly goal's record looks back this many months before the current one. */
        const val MONTHS = 5
    }
}
