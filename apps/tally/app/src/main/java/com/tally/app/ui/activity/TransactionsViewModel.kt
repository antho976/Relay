package com.tally.app.ui.activity

import androidx.compose.runtime.Immutable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.TransactionRow
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerFilter
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.app.ui.nav.Args
import com.tally.core.AccountType
import com.tally.core.BudgetPeriod
import com.tally.core.CategoryKind
import com.tally.core.PaceReading
import com.tally.core.TxType
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import java.time.LocalDate
import javax.inject.Inject

/** What went in and out through one account over a set of entries. */
@Immutable
data class AccountFlows(
    val moneyIn: Long = 0,
    val moneyOut: Long = 0,
    val inCount: Int = 0,
    val outCount: Int = 0,
)

/**
 * In and out for [accountId]: income into it and transfers to it come in; expenses from it and
 * transfers out of it go out.
 */
internal fun accountFlows(rows: List<TransactionRow>, accountId: Long): AccountFlows {
    var moneyIn = 0L
    var moneyOut = 0L
    var inCount = 0
    var outCount = 0
    rows.forEach { r ->
        val incoming = when (r.type) {
            TxType.INCOME -> r.accountId == accountId
            TxType.TRANSFER -> r.toAccountId == accountId
            TxType.EXPENSE -> false
        }
        val outgoing = when (r.type) {
            TxType.EXPENSE, TxType.TRANSFER -> r.accountId == accountId
            TxType.INCOME -> false
        }
        if (incoming) { moneyIn += r.amount; inCount++ }
        if (outgoing) { moneyOut += r.amount; outCount++ }
    }
    return AccountFlows(moneyIn, moneyOut, inCount, outCount)
}

@Immutable
data class TransactionsState(
    val today: LocalDate,
    val period: BudgetPeriod,
    val categoryId: Long = 0,
    val accountId: Long = 0,
    /** Null until loaded, and after the category or account was deleted. */
    val name: String? = null,
    val icon: String? = null,
    val color: Int? = null,
    val categoryKind: CategoryKind = CategoryKind.EXPENSE,
    val accountType: AccountType = AccountType.CHEQUING,
    /** Every entry it holds, all time. */
    val entryCount: Int = 0,
    val balance: Long = 0,
    val openingBalance: Long = 0,
    /** The category's standing monthly budget; null without one (and always for income). */
    val budget: Long? = null,
    val reading: PaceReading? = null,
    /** Category: its total this period, its entries this period, and the period before. */
    val periodTotal: Long = 0,
    val periodCount: Int = 0,
    val lastPeriodTotal: Long = 0,
    val lastPeriodSameDay: Long = 0,
    /** Account: this period's money through it. */
    val flows: AccountFlows = AccountFlows(),
    /** The settled query these rows are for. */
    val query: String = "",
    val days: List<DayGroup> = emptyList(),
    val summary: LedgerSummary = LedgerSummary(),
    val truncated: Boolean = false,
    val loaded: Boolean = false,
) {
    val isAccount: Boolean get() = categoryId == 0L && accountId != 0L
    val missing: Boolean get() = loaded && name == null
    val searching: Boolean get() = query.isNotBlank()
}

/**
 * The ledger for ONE category or ONE account, all time, with this period's reading on top: the
 * budget's pace for a category, the money through it for an account.
 */
@OptIn(ExperimentalCoroutinesApi::class, FlowPreview::class)
@HiltViewModel
class TransactionsViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val ledger: LedgerRepository,
    private val plan: PlanRepository,
    settings: SettingsRepository,
    private val clock: Clock,
) : ViewModel() {

    private val categoryId: Long = savedStateHandle.get<Long>(Args.CATEGORY) ?: 0L
    private val accountId: Long = if (categoryId != 0L) 0L else savedStateHandle.get<Long>(Args.ACCOUNT) ?: 0L

    private val today = MutableStateFlow(clock.today())
    private val query = MutableStateFlow("")

    fun onResume() { today.value = clock.today() }
    fun setQuery(text: String) { query.value = text }

    private data class Subject(
        val name: String,
        val icon: String? = null,
        val color: Int? = null,
        val kind: CategoryKind = CategoryKind.EXPENSE,
        val accountType: AccountType = AccountType.CHEQUING,
        val entryCount: Int = 0,
        val balance: Long = 0,
        val openingBalance: Long = 0,
    )

    private data class Figures(
        val periodTotal: Long = 0,
        val periodCount: Int = 0,
        val lastPeriodTotal: Long = 0,
        val lastPeriodSameDay: Long = 0,
        val budget: Long? = null,
        val reading: PaceReading? = null,
        val flows: AccountFlows = AccountFlows(),
    )

    // Observed rather than read once, so an edit made from this screen shows on coming back.
    private val subject: Flow<Subject?> = when {
        categoryId != 0L -> ledger.categoriesWithCounts().map { list ->
            list.firstOrNull { it.category.id == categoryId }?.let { c ->
                Subject(
                    name = c.category.name,
                    icon = c.category.icon,
                    color = c.category.color,
                    kind = c.category.kind,
                    entryCount = c.entryCount,
                )
            }
        }
        accountId != 0L -> ledger.balances().map { list ->
            list.firstOrNull { it.id == accountId }?.let { a ->
                Subject(
                    name = a.name,
                    accountType = a.type,
                    entryCount = a.entryCount,
                    balance = a.balance,
                    openingBalance = a.openingBalance,
                )
            }
        }
        else -> flowOf<Subject?>(null)
    }.distinctUntilChanged()

    private val period = combine(settings.settings, today) { s, d -> s.periodFor(d) to d }.distinctUntilChanged()

    private val kind = subject.map { it?.kind ?: CategoryKind.EXPENSE }.distinctUntilChanged()

    private val figures: Flow<Figures> = combine(period, kind) { p, k -> p to k }.flatMapLatest { (pd, k) ->
        val (p, day) = pd
        when {
            categoryId != 0L -> categoryFigures(p, day, k)
            accountId != 0L -> ledger
                .search(LedgerFilter(accountId = accountId, start = p.start, end = p.endExclusive), limit = PERIOD_LIMIT)
                .map { Figures(flows = accountFlows(it, accountId)) }
            else -> flowOf(Figures())
        }
    }

    private fun categoryFigures(p: BudgetPeriod, day: LocalDate, kind: CategoryKind): Flow<Figures> {
        val type = if (kind == CategoryKind.INCOME) TxType.INCOME else TxType.EXPENSE
        val previous = p.shift(-1)
        val previousSameDay = minOf(previous.start.plusDays(p.elapsedDays(day).toLong()), previous.endExclusive)
        return combine(
            ledger.byCategory(type, p.start, p.endExclusive),
            ledger.byCategory(type, previous.start, previous.endExclusive),
            ledger.byCategory(type, previous.start, previousSameDay),
            plan.budgets(),
        ) { now, last, lastSameDay, budgets ->
            val mine = now.firstOrNull { it.categoryId == categoryId }
            val total = mine?.total ?: 0L
            val budget = if (type == TxType.EXPENSE) budgets.firstOrNull { it.categoryId == categoryId }?.amount else null
            Figures(
                periodTotal = total,
                periodCount = mine?.count ?: 0,
                lastPeriodTotal = last.firstOrNull { it.categoryId == categoryId }?.total ?: 0L,
                lastPeriodSameDay = lastSameDay.firstOrNull { it.categoryId == categoryId }?.total ?: 0L,
                budget = budget,
                reading = budget?.let { PaceReading(it, total, p.days, p.elapsedDays(day)) },
            )
        }
    }

    /** The settled query, the capped rows it lists, and the summary of EVERY entry it matches. */
    private data class Results(val query: String, val rows: List<TransactionRow>, val summary: LedgerSummary)

    private val results: Flow<Results> = query
        .debounce { if (it.isBlank()) 0L else SEARCH_DEBOUNCE_MS }
        .map { it.trim() }
        .distinctUntilChanged()
        .flatMapLatest { q ->
            if (categoryId == 0L && accountId == 0L) {
                flowOf(Results(q, emptyList(), LedgerSummary()))
            } else {
                val filter = LedgerFilter(
                    query = q,
                    categoryId = categoryId.takeIf { it != 0L },
                    accountId = accountId.takeIf { it != 0L },
                )
                combine(ledger.search(filter), ledger.summary(filter)) { rows, summary -> Results(q, rows, summary) }
            }
        }

    val state: StateFlow<TransactionsState> = combine(subject, figures, results, period) { s, f, r, pd ->
        val q = r.query
        val rows = r.rows
        val (p, day) = pd
        TransactionsState(
            today = day,
            period = p,
            categoryId = categoryId,
            accountId = accountId,
            name = s?.name,
            icon = s?.icon,
            color = s?.color,
            categoryKind = s?.kind ?: CategoryKind.EXPENSE,
            accountType = s?.accountType ?: AccountType.CHEQUING,
            entryCount = s?.entryCount ?: 0,
            balance = s?.balance ?: 0L,
            openingBalance = s?.openingBalance ?: 0L,
            budget = f.budget,
            reading = f.reading,
            periodTotal = f.periodTotal,
            periodCount = f.periodCount,
            lastPeriodTotal = f.lastPeriodTotal,
            lastPeriodSameDay = f.lastPeriodSameDay,
            flows = f.flows,
            query = q,
            days = groupByDay(rows),
            summary = r.summary,
            truncated = rows.size >= LedgerRepository.SEARCH_LIMIT,
            loaded = true,
        )
    }.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5_000),
        TransactionsState(
            today = clock.today(),
            period = BudgetPeriod.containing(clock.today()),
            categoryId = categoryId,
            accountId = accountId,
        ),
    )

    private companion object {
        /** Enough for any one period of one account; the in and out must not be capped like the list. */
        const val PERIOD_LIMIT = 100_000
    }
}
