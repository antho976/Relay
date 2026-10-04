package com.tally.app.ui.plan

import androidx.compose.runtime.Immutable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.BudgetEntity
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.app.ui.common.DRAFT_KEY
import com.tally.app.ui.common.Notices
import com.tally.app.ui.nav.Args
import com.tally.core.AmountInput
import com.tally.core.BudgetPeriod
import com.tally.core.MoneyFormatter
import com.tally.core.PaceReading
import com.tally.core.TxType
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.util.Locale
import javax.inject.Inject

/** How many periods the history reads: the current one and the three before it. */
internal const val HISTORY_PERIODS = 4

/** Everything the budget editor draws. Readings are folded here, not in composition. */
@Immutable
data class BudgetEditState(
    /** 0 is the overall monthly budget. */
    val categoryId: Long,
    val today: LocalDate,
    val period: BudgetPeriod,
    val startDay: Int = 1,
    val loaded: Boolean = false,
    /** The category's name; null for the overall budget. */
    val name: String? = null,
    val icon: String? = null,
    val color: Int? = null,
    /** The budget as stored; null when there is none yet. */
    val existing: Long? = null,
    val input: AmountInput = AmountInput(),
    /** Spent per period, oldest first; the last is the current period so far. */
    val history: List<Long> = List(HISTORY_PERIODS) { 0L },
    /** Each history period's first day, same order. */
    val historyStarts: List<LocalDate> = emptyList(),
    val lastMonth: Long = 0,
    /** The mean of the three completed periods that had spending; null with none on record. */
    val average: Long? = null,
    val monthsOnRecord: Int = 0,
    /** The current period read against what is typed. */
    val reading: PaceReading,
    /** Last month rounded up to the nearest ten, offered as a fill. */
    val suggestLast: Long? = null,
    /** The average rounded to the nearest ten, offered as a fill. */
    val suggestAverage: Long? = null,
) {
    val isOverall: Boolean get() = categoryId == BudgetEntity.OVERALL
    val thisMonth: Long get() = history.lastOrNull() ?: 0L
}

/**
 * The budget editor: one standing monthly limit for a category (or the overall one, id 0), typed
 * on the keypad, read against what the category spent this month and the three before.
 */
@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class BudgetEditViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val plan: PlanRepository,
    private val ledger: LedgerRepository,
    private val settings: SettingsRepository,
    private val notices: Notices,
    clock: Clock,
) : ViewModel() {

    private val categoryId: Long = savedStateHandle.get<Long>(Args.CATEGORY) ?: 0L
    private val today: LocalDate = clock.today()

    private val input = MutableStateFlow(AmountInput())
    private val ready = MutableStateFlow(false)
    private val finished = Channel<Unit>(Channel.CONFLATED)

    /** Fires once after a save or a removal; the route goes back on it. */
    val done: Flow<Unit> = finished.receiveAsFlow()

    /** One write per visit. */
    private var busy = false

    private fun spentIn(p: BudgetPeriod): Flow<Long> =
        if (categoryId == BudgetEntity.OVERALL) {
            ledger.typeTotals(p.start, p.endExclusive).map { rows -> rows.firstOrNull { it.type == TxType.EXPENSE }?.total ?: 0L }
        } else {
            ledger.byCategory(TxType.EXPENSE, p.start, p.endExclusive).map { rows -> rows.firstOrNull { it.categoryId == categoryId }?.total ?: 0L }
        }

    private data class History(val period: BudgetPeriod, val startDay: Int, val values: List<Long>, val starts: List<LocalDate>)

    private val history: Flow<History> = settings.settings
        .map { it.periodFor(today) to it.monthStartDay }
        .distinctUntilChanged()
        .flatMapLatest { (period, startDay) ->
            val periods = (HISTORY_PERIODS - 1 downTo 0).map { period.shift(-it.toLong()) }
            combine(periods.map { spentIn(it) }) { values ->
                History(period, startDay, values.toList(), periods.map { it.start })
            }
        }

    val state: StateFlow<BudgetEditState> = combine(
        history,
        ledger.categories(),
        plan.budgets(),
        input,
        ready,
    ) { h, categories, budgets, typed, isReady ->
        val category = if (categoryId == BudgetEntity.OVERALL) null else categories.firstOrNull { it.id == categoryId }
        val completed = h.values.dropLast(1)
        val last = completed.lastOrNull() ?: 0L
        val average = averageOnRecord(completed)
        val step = 10 * MoneyFormatter.pow10(typed.fractionDigits)
        val thisMonth = h.values.lastOrNull() ?: 0L
        BudgetEditState(
            categoryId = categoryId,
            today = today,
            period = h.period,
            startDay = h.startDay,
            loaded = isReady,
            name = category?.name ?: if (categoryId == BudgetEntity.OVERALL) null else "Category",
            icon = category?.icon,
            color = category?.color,
            existing = budgets.firstOrNull { it.categoryId == categoryId }?.amount,
            input = typed,
            history = h.values,
            historyStarts = h.starts,
            lastMonth = last,
            average = average,
            monthsOnRecord = completed.count { it > 0L },
            reading = PaceReading(typed.minor, thisMonth, h.period.days, h.period.elapsedDays(today)),
            suggestLast = roundUpTo(last, step).takeIf { it > 0L },
            suggestAverage = average?.let { roundTo(it, step) }?.takeIf { it > 0L },
        )
    }.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5_000),
        BudgetEditState(
            categoryId = categoryId,
            today = today,
            period = BudgetPeriod.containing(today),
            reading = PaceReading(0L, 0L, BudgetPeriod.containing(today).days, 0),
        ),
    )

    init {
        viewModelScope.launch {
            // What was typed before the process died wins over the stored limit.
            val kept = savedStateHandle.get<String>(DRAFT_KEY)
            val s = settings.current()
            // The keypad works in the owner's currency's minor units (0 digits for JPY).
            val digits = MoneyFormatter(s.currency, Locale.getDefault()).fractionDigits
            val stored = plan.budgetFor(categoryId)
            input.value = when {
                kept != null -> AmountInput(kept, digits)
                stored != null -> AmountInput.of(stored.amount, digits)
                else -> AmountInput(fractionDigits = digits)
            }
            ready.value = true
            input.collect { savedStateHandle[DRAFT_KEY] = it.text }
        }
    }

    fun press(key: PadKey) = input.update { it.pressed(key) }

    /** A quick fill from the history chips. */
    fun fill(amount: Long) = input.update { AmountInput.of(amount, it.fractionDigits) }

    fun save() {
        if (busy) return
        val amount = input.value.minor
        if (amount <= 0L) return
        busy = true
        viewModelScope.launch {
            plan.setBudget(categoryId, amount)
            finished.send(Unit)
        }
    }

    /** Removes at once and offers Undo; nothing asks first. */
    fun remove() {
        if (busy) return
        busy = true
        val repo = plan
        val id = categoryId
        // Named, so the snackbar says which limit its Undo brings back.
        val line = when {
            id == BudgetEntity.OVERALL -> "Monthly budget removed"
            else -> state.value.name?.let { "$it budget removed" } ?: "Budget removed"
        }
        viewModelScope.launch {
            val old = repo.budgetFor(id)?.amount
            if (old == null) {
                busy = false
                return@launch
            }
            repo.setBudget(id, 0L)
            // The undo holds the repository and the amount, never this ViewModel.
            notices.showUndo(line) { repo.setBudget(id, old) }
            finished.send(Unit)
        }
    }
}
