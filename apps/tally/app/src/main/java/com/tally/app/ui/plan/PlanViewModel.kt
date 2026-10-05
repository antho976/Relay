package com.tally.app.ui.plan

import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.core.BudgetPeriod
import com.tally.core.TxType
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import java.time.LocalDate
import javax.inject.Inject

/** Everything the Plan tab draws, its three lenses folded in the ViewModel. */
@Immutable
data class PlanState(
    val today: LocalDate,
    val period: BudgetPeriod,
    val loaded: Boolean = false,
    val budgets: BudgetsReading = BudgetsReading(),
    val bills: BillsReading = BillsReading(),
    val goals: GoalsReading = GoalsReading(),
) {
    val daysLeft: Int get() = period.daysLeft(today)
}

@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class PlanViewModel @Inject constructor(
    plan: PlanRepository,
    ledger: LedgerRepository,
    settings: SettingsRepository,
    goalSource: GoalSource,
    private val clock: Clock,
) : ViewModel() {

    /** Re-read on resume, so a phone left on Plan overnight shows the new day. */
    private val today = MutableStateFlow(clock.today())

    fun onResume() { today.value = clock.today() }

    private val context = combine(settings.settings, today) { s, d -> s.periodFor(d) to d }.distinctUntilChanged()

    val state: StateFlow<PlanState> = context.flatMapLatest { (period, day) ->
        val previous = period.shift(-1)
        val budgets = combine(
            plan.budgets(),
            ledger.byCategory(TxType.EXPENSE, period.start, period.endExclusive),
            ledger.typeTotals(period.start, period.endExclusive),
            ledger.typeTotals(previous.start, previous.endExclusive),
            ledger.categories(),
        ) { rows, spending, totals, lastTotals, categories ->
            budgetsReading(period, day, rows, spending, totals, lastTotals, categories)
        }
        val bills = plan.recurring().map { rows -> billsReading(rows, day) }
        val goals = goalSource.reading(period, day)
        combine(budgets, bills, goals) { b, r, g ->
            PlanState(today = day, period = period, loaded = true, budgets = b, bills = r, goals = g)
        }
    }.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5_000),
        PlanState(today = clock.today(), period = BudgetPeriod.containing(clock.today())),
    )
}
