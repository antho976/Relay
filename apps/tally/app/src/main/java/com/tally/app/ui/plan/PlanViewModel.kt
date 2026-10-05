package com.tally.app.ui.plan

import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.app.ui.common.Notices
import com.tally.core.BudgetPeriod
import com.tally.core.Copy
import com.tally.core.MoneyFormatter
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
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.util.Locale
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
    private val plan: PlanRepository,
    ledger: LedgerRepository,
    settings: SettingsRepository,
    goalSource: GoalSource,
    private val notices: Notices,
    private val clock: Clock,
) : ViewModel() {

    /** Re-read on resume, so a phone left on Plan overnight shows the new day. */
    private val today = MutableStateFlow(clock.today())

    fun onResume() { today.value = clock.today() }

    private data class Context(val period: BudgetPeriod, val day: LocalDate, val step: Long)

    private val context = combine(settings.settings, today) { s, d ->
        // Suggested budgets round up to tens of the currency's whole unit.
        val digits = MoneyFormatter(s.currency, Locale.getDefault()).fractionDigits
        Context(s.periodFor(d), d, 10 * MoneyFormatter.pow10(digits))
    }.distinctUntilChanged()

    val state: StateFlow<PlanState> = context.flatMapLatest { (period, day, step) ->
        val previous = period.shift(-1)
        val history = combine(
            (SUGGEST_PERIODS downTo 1).map { back ->
                val p = period.shift(-back.toLong())
                ledger.byCategory(TxType.EXPENSE, p.start, p.endExclusive)
            }
        ) { periods -> periods.toList() }
        val spending = combine(
            ledger.byCategory(TxType.EXPENSE, period.start, period.endExclusive),
            history,
        ) { now, before -> now to before }
        val budgets = combine(
            plan.budgets(),
            spending,
            ledger.typeTotals(period.start, period.endExclusive),
            ledger.typeTotals(previous.start, previous.endExclusive),
            ledger.categories(),
        ) { rows, (now, before), totals, lastTotals, categories ->
            budgetsReading(period, day, rows, now, totals, lastTotals, categories, before, step)
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

    /** Sets every suggested budget at once; the notice offers the undo. */
    fun applySuggestions() {
        val suggestions = state.value.budgets.suggestions
        if (suggestions.isEmpty()) return
        viewModelScope.launch {
            plan.setBudgets(suggestions.associate { it.categoryId to it.amount })
            notices.showUndo(Copy.plural(suggestions.size, "budget") + " set from your averages") {
                plan.setBudgets(suggestions.associate { it.categoryId to 0L })
            }
        }
    }
}
