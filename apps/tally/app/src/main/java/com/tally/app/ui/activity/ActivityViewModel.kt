package com.tally.app.ui.activity

import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.TransactionRow
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerFilter
import com.tally.app.data.repo.LedgerRepository
import com.tally.core.BudgetPeriod
import com.tally.core.TxType
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import java.time.LocalDate
import javax.inject.Inject

/** How long typing settles before the ledger is searched. Clearing the field never waits. */
internal const val SEARCH_DEBOUNCE_MS = 150L

/** One day of the ledger: its entries newest first, and what went out and came in that day. */
@Immutable
data class DayGroup(
    val date: LocalDate,
    val rows: List<TransactionRow>,
    val spent: Long,
    val income: Long,
)

/**
 * The readings over whatever set of entries is on screen. [basis] is the type the largest and
 * average readings are about: spending when there is any, else income, else transfers, so a
 * salary never stands in as the "largest spend" and a card payment never dwarfs a month of coffee.
 */
@Immutable
data class LedgerSummary(
    val count: Int = 0,
    val spent: Long = 0,
    val income: Long = 0,
    val moved: Long = 0,
    val transfers: Int = 0,
    val activeDays: Int = 0,
    val basis: TxType = TxType.EXPENSE,
    val basisCount: Int = 0,
    val largest: TransactionRow? = null,
    val average: Long = 0,
) {
    val net: Long get() = income - spent
}

/** The Activity lens. [type] null shows every entry. */
enum class ActivityFilter(val label: String, val type: TxType?) {
    ALL("All", null),
    SPENT("Spent", TxType.EXPENSE),
    INCOME("Income", TxType.INCOME),
    TRANSFERS("Transfers", TxType.TRANSFER),
}

@Immutable
data class ActivityState(
    val today: LocalDate,
    val period: BudgetPeriod,
    val isCurrentPeriod: Boolean = true,
    /** The (settled, trimmed) query these results are for; blank browses [period]. */
    val query: String = "",
    val filter: ActivityFilter = ActivityFilter.ALL,
    val days: List<DayGroup> = emptyList(),
    val summary: LedgerSummary = LedgerSummary(),
    /** Spent per day of the period so far (every day of a past one); null while searching all time. */
    val perDay: Long? = null,
    /** The search hit its cap: only the newest [LedgerRepository.SEARCH_LIMIT] are shown. */
    val truncated: Boolean = false,
    val loaded: Boolean = false,
) {
    val searching: Boolean get() = query.isNotBlank()
}

/**
 * Rows, newest first as the search returns them, folded into one group per day, newest day
 * first. Each day keeps its rows in the order they came and carries its own in and out.
 */
internal fun groupByDay(rows: List<TransactionRow>): List<DayGroup> =
    rows.groupBy { it.date }
        .map { (date, list) ->
            DayGroup(
                date = date,
                rows = list,
                spent = list.sumOf { if (it.type == TxType.EXPENSE) it.amount else 0L },
                income = list.sumOf { if (it.type == TxType.INCOME) it.amount else 0L },
            )
        }
        .sortedByDescending { it.date }

/** Totals, counts, and the largest and average entry of [rows]. */
internal fun summarize(rows: List<TransactionRow>): LedgerSummary {
    if (rows.isEmpty()) return LedgerSummary()
    var spent = 0L
    var income = 0L
    var moved = 0L
    var expenses = 0
    var incomes = 0
    var transfers = 0
    rows.forEach {
        when (it.type) {
            TxType.EXPENSE -> { spent += it.amount; expenses++ }
            TxType.INCOME -> { income += it.amount; incomes++ }
            TxType.TRANSFER -> { moved += it.amount; transfers++ }
        }
    }
    val basis = when {
        expenses > 0 -> TxType.EXPENSE
        incomes > 0 -> TxType.INCOME
        else -> TxType.TRANSFER
    }
    val pool = rows.filter { it.type == basis }
    return LedgerSummary(
        count = rows.size,
        spent = spent,
        income = income,
        moved = moved,
        transfers = transfers,
        activeDays = rows.asSequence().map { it.date }.distinct().count(),
        basis = basis,
        basisCount = pool.size,
        largest = pool.maxByOrNull { it.amount },
        average = if (pool.isEmpty()) 0L else pool.sumOf { it.amount } / pool.size,
    )
}

/**
 * The ledger browser: one period at a time, or every month while a query is typed, narrowed by
 * type. Grouping and every total happen here, never in composition.
 */
@OptIn(ExperimentalCoroutinesApi::class, FlowPreview::class)
@HiltViewModel
class ActivityViewModel @Inject constructor(
    private val ledger: LedgerRepository,
    settings: SettingsRepository,
    private val clock: Clock,
) : ViewModel() {

    /** Re-read on resume, so a phone left on Activity overnight shows the new day. */
    private val today = MutableStateFlow(clock.today())

    /** 0 is the current period, -1 the one before. Never ahead of today. */
    private val periodOffset = MutableStateFlow(0)
    private val query = MutableStateFlow("")
    private val filter = MutableStateFlow(ActivityFilter.ALL)

    fun onResume() { today.value = clock.today() }
    fun previousPeriod() { periodOffset.update { it - 1 } }
    fun nextPeriod() { periodOffset.update { (it + 1).coerceAtMost(0) } }
    fun setQuery(text: String) { query.value = text }
    fun setFilter(f: ActivityFilter) { filter.value = f }

    private data class Inputs(
        val period: BudgetPeriod,
        val current: Boolean,
        val day: LocalDate,
        val query: String,
        val filter: ActivityFilter,
    )

    private val settledQuery = query
        .debounce { if (it.isBlank()) 0L else SEARCH_DEBOUNCE_MS }
        .map { it.trim() }
        .distinctUntilChanged()

    private val inputs = combine(settings.settings, today, periodOffset, settledQuery, filter) { s, day, offset, q, f ->
        Inputs(s.periodFor(day).shift(offset.toLong()), offset >= 0, day, q, f)
    }.distinctUntilChanged()

    val state: StateFlow<ActivityState> = inputs.flatMapLatest { i ->
        val searching = i.query.isNotEmpty()
        val ledgerFilter = LedgerFilter(
            query = i.query,
            type = i.filter.type,
            start = if (searching) null else i.period.start,
            end = if (searching) null else i.period.endExclusive,
        )
        ledger.search(ledgerFilter).map { rows ->
            val summary = summarize(rows)
            val days = when {
                searching -> 0
                i.current -> i.period.elapsedDays(i.day)
                else -> i.period.days
            }
            ActivityState(
                today = i.day,
                period = i.period,
                isCurrentPeriod = i.current,
                query = i.query,
                filter = i.filter,
                days = groupByDay(rows),
                summary = summary,
                perDay = if (days > 0) summary.spent / days else null,
                truncated = rows.size >= LedgerRepository.SEARCH_LIMIT,
                loaded = true,
            )
        }
    }.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5_000),
        ActivityState(today = clock.today(), period = BudgetPeriod.containing(clock.today())),
    )
}
