package com.margin.app.ui.home

import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.margin.app.data.Clock
import com.margin.app.data.db.AccountBalance
import com.margin.app.data.db.BudgetEntity
import com.margin.app.data.db.TransactionRow
import com.margin.app.data.prefs.SettingsRepository
import com.margin.app.data.repo.LedgerRepository
import com.margin.app.data.repo.PlanRepository
import com.margin.core.BudgetPeriod
import com.margin.core.PaceReading
import com.margin.core.TxType
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
import java.time.temporal.ChronoUnit
import javax.inject.Inject

@Immutable
data class Envelope(
    val categoryId: Long,
    val name: String,
    val icon: String?,
    val color: Int?,
    val reading: PaceReading,
)

@Immutable
data class UpcomingBill(
    val id: Long,
    val name: String,
    val amount: Long,
    val type: TxType,
    val date: LocalDate,
    val daysUntil: Long,
    val icon: String?,
    val color: Int?,
    val accountName: String,
    val autoPost: Boolean,
)

@Immutable
data class HomeState(
    val today: LocalDate,
    val period: BudgetPeriod,
    val income: Long = 0,
    val spent: Long = 0,
    /** The overall monthly budget's reading; null when no overall budget is set. */
    val reading: PaceReading? = null,
    /** Category budgets, the furthest over pace first. */
    val envelopes: List<Envelope> = emptyList(),
    val upcoming: List<UpcomingBill> = emptyList(),
    val recent: List<TransactionRow> = emptyList(),
    val accounts: List<AccountBalance> = emptyList(),
    val hasEntries: Boolean = false,
    val sampleLoaded: Boolean = false,
    val loaded: Boolean = false,
) {
    val daysLeft: Int get() = period.daysLeft(today)
    val net: Long get() = accounts.filter { !it.archived }.sumOf { it.balance }
}

@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class HomeViewModel @Inject constructor(
    settings: SettingsRepository,
    ledger: LedgerRepository,
    plan: PlanRepository,
    private val clock: Clock,
) : ViewModel() {

    /** Re-read on resume, so a phone left on Home overnight shows the new day. */
    private val today = MutableStateFlow(clock.today())

    fun onResume() { today.value = clock.today() }

    private val context = combine(settings.settings, today) { s, d -> Triple(s.periodFor(d), d, s.sampleLoaded) }
        .distinctUntilChanged()

    val state: StateFlow<HomeState> = context.flatMapLatest { (period, day, sample) ->
        val totals = ledger.typeTotals(period.start, period.endExclusive)
        val byCategory = ledger.byCategory(TxType.EXPENSE, period.start, period.endExclusive)
        val money = combine(totals, byCategory, plan.budgets()) { t, cats, budgets ->
            val income = t.firstOrNull { it.type == TxType.INCOME }?.total ?: 0L
            val spent = t.firstOrNull { it.type == TxType.EXPENSE }?.total ?: 0L
            val spentBy = cats.associate { (it.categoryId ?: -1L) to it.total }
            val elapsed = period.elapsedDays(day)
            val overall = budgets.firstOrNull { it.categoryId == BudgetEntity.OVERALL }
                ?.let { PaceReading(it.amount, spent, period.days, elapsed) }
            val envelopes = budgets.filter { it.categoryId != BudgetEntity.OVERALL && it.categoryName != null }.map {
                Envelope(
                    categoryId = it.categoryId,
                    name = it.categoryName.orEmpty(),
                    icon = it.categoryIcon,
                    color = it.categoryColor,
                    reading = PaceReading(it.amount, spentBy[it.categoryId] ?: 0L, period.days, elapsed),
                )
            }.sortedByDescending { it.reading.paceDelta.toDouble() / it.reading.budget.coerceAtLeast(1) }
            Triple(income to spent, overall, envelopes)
        }
        val upcoming = plan.recurring().map { rows ->
            rows.asSequence()
                .filter { it.recurring.active }
                .map { r ->
                    UpcomingBill(
                        id = r.recurring.id,
                        name = r.recurring.name,
                        amount = r.recurring.amount,
                        type = r.recurring.type,
                        date = r.recurring.nextDate,
                        daysUntil = ChronoUnit.DAYS.between(day, r.recurring.nextDate),
                        icon = r.categoryIcon,
                        color = r.categoryColor,
                        accountName = r.accountName,
                        autoPost = r.recurring.autoPost,
                    )
                }
                .filter { it.daysUntil <= UPCOMING_DAYS }
                .sortedBy { it.date }
                .take(3)
                .toList()
        }
        combine(money, upcoming, ledger.recent(RECENT), ledger.balances(), ledger.count()) { m, bills, recent, accounts, count ->
            HomeState(
                today = day,
                period = period,
                income = m.first.first,
                spent = m.first.second,
                reading = m.second,
                envelopes = m.third,
                upcoming = bills,
                recent = recent,
                accounts = accounts,
                hasEntries = count > 0,
                sampleLoaded = sample,
                loaded = true,
            )
        }
    }.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5_000),
        HomeState(today = clock.today(), period = BudgetPeriod.containing(clock.today())),
    )

    companion object {
        const val RECENT = 5
        const val UPCOMING_DAYS = 14L
    }
}
