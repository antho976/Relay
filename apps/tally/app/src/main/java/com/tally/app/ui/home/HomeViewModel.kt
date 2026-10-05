package com.tally.app.ui.home

import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.BudgetEntity
import com.tally.app.data.db.TransactionRow
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.app.ui.plan.GoalLine
import com.tally.app.ui.plan.GoalSource
import com.tally.app.ui.plan.GoalStatus
import com.tally.core.BudgetPeriod
import com.tally.core.PaceReading
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
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.temporal.TemporalAdjusters
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

/**
 * Furthest over pace first, measured against each budget's size so a small envelope can lead.
 * Plan's sortEnvelopes keeps the same rule, so both tabs list the envelopes in one order.
 */
internal fun sortEnvelopes(envelopes: List<Envelope>): List<Envelope> =
    envelopes.sortedByDescending { it.reading.paceDelta.toDouble() / it.reading.budget.coerceAtLeast(1) }

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
    /** Spent per day of the current week, in display order (week start first). */
    val week: List<Long> = List(7) { 0L },
    /** Defaults to the Sunday on or before [today]; the view model passes the owner's week start. */
    val weekStart: LocalDate = today.with(TemporalAdjusters.previousOrSame(DayOfWeek.SUNDAY)),
    /** What the previous period had spent by the same day, for "vs last month". */
    val lastPeriodSameDay: Long = 0,
    val lastPeriodIncomeSameDay: Long = 0,
    /** Up to [HomeViewModel.GOALS] goals, the ones that need attention first. */
    val goals: List<GoalLine> = emptyList(),
) {
    val todayInWeek: Int get() = ChronoUnit.DAYS.between(weekStart, today).toInt().coerceIn(0, 6)
    val weekSpent: Long get() = week.take(todayInWeek + 1).sum()
    /** The week's leading days that fall before [period] began: early in a month, the week opens in the last one. */
    val weekDaysBeforePeriod: Int get() = ChronoUnit.DAYS.between(weekStart, period.start).toInt().coerceIn(0, 7)
    /** What those days spent, so the week's figure and the month's Out add up on screen. */
    val weekSpentBeforePeriod: Long get() = week.take(minOf(weekDaysBeforePeriod, todayInWeek + 1)).sum()
    /** An even day's share of the overall budget, the dashed line on the week bars. */
    val dailyBudget: Long? get() = reading?.let { it.budget / period.days }
    val daysLeft: Int get() = period.daysLeft(today)
    val net: Long get() = accounts.filter { !it.archived }.sumOf { it.balance }
}

/** Home's goals: the ones behind their pace first, then the ones still open, reached ones last. */
internal fun homeGoals(goals: List<GoalLine>): List<GoalLine> =
    goals.sortedBy {
        when (it.status) {
            GoalStatus.BEHIND -> 0
            GoalStatus.ON_TRACK, GoalStatus.OPEN -> 1
            GoalStatus.REACHED -> 2
        }
    }.take(HomeViewModel.GOALS)

@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class HomeViewModel @Inject constructor(
    settings: SettingsRepository,
    ledger: LedgerRepository,
    plan: PlanRepository,
    goalSource: GoalSource,
    private val clock: Clock,
) : ViewModel() {

    /** Re-read on resume, so a phone left on Home overnight shows the new day. */
    private val today = MutableStateFlow(clock.today())

    fun onResume() { today.value = clock.today() }

    private data class Context(val period: BudgetPeriod, val day: LocalDate, val sample: Boolean, val weekStart: LocalDate)

    private val context = combine(settings.settings, today) { s, d ->
        val first = if (s.weekStartsMonday) DayOfWeek.MONDAY else DayOfWeek.SUNDAY
        Context(s.periodFor(d), d, s.sampleLoaded, d.with(TemporalAdjusters.previousOrSame(first)))
    }.distinctUntilChanged()

    val state: StateFlow<HomeState> = context.flatMapLatest { (period, day, sample, weekStart) ->
        val totals = ledger.typeTotals(period.start, period.endExclusive)
        val byCategory = ledger.byCategory(TxType.EXPENSE, period.start, period.endExclusive)
        val money = combine(totals, byCategory, plan.budgets()) { t, cats, budgets ->
            val income = t.firstOrNull { it.type == TxType.INCOME }?.total ?: 0L
            val spent = t.firstOrNull { it.type == TxType.EXPENSE }?.total ?: 0L
            val spentBy = cats.associate { (it.categoryId ?: -1L) to it.total }
            val elapsed = period.elapsedDays(day)
            val overall = budgets.firstOrNull { it.categoryId == BudgetEntity.OVERALL }
                ?.let { PaceReading(it.amount, spent, period.days, elapsed) }
            val envelopes = sortEnvelopes(
                budgets.filter { it.categoryId != BudgetEntity.OVERALL && it.categoryName != null }.map {
                    Envelope(
                        categoryId = it.categoryId,
                        name = it.categoryName.orEmpty(),
                        icon = it.categoryIcon,
                        color = it.categoryColor,
                        reading = PaceReading(it.amount, spentBy[it.categoryId] ?: 0L, period.days, elapsed),
                    )
                }
            )
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
        val previous = period.shift(-1)
        val previousSameDay = previous.start.plusDays(period.elapsedDays(day).toLong())
        val extras = combine(
            ledger.dailyExpense(weekStart, weekStart.plusDays(7)),
            ledger.typeTotals(previous.start, minOf(previousSameDay, previous.endExclusive)),
        ) { days, last ->
            val byDay = days.associate { it.date to it.total }
            Triple(
                List(7) { byDay[weekStart.plusDays(it.toLong())] ?: 0L },
                last.firstOrNull { it.type == TxType.EXPENSE }?.total ?: 0L,
                last.firstOrNull { it.type == TxType.INCOME }?.total ?: 0L,
            )
        }
        val base = combine(money, upcoming, ledger.recent(RECENT), ledger.balances(), ledger.count()) { m, bills, recent, accounts, count ->
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
                weekStart = weekStart,
            )
        }
        combine(base, extras, goalSource.reading(period, day)) { b, x, g ->
            b.copy(week = x.first, lastPeriodSameDay = x.second, lastPeriodIncomeSameDay = x.third, goals = homeGoals(g.goals))
        }
    }.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5_000),
        HomeState(today = clock.today(), period = BudgetPeriod.containing(clock.today())),
    )

    companion object {
        const val RECENT = 6
        const val UPCOMING_DAYS = 14L
        const val GOALS = 3
    }
}
