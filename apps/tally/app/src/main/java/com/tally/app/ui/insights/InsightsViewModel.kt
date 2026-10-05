package com.tally.app.ui.insights

import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.db.BudgetEntity
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.CategoryTotal
import com.tally.app.data.db.DayTotal
import com.tally.app.data.db.PayeeTotal
import com.tally.app.data.db.TransactionRow
import com.tally.app.data.db.TypeTotal
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.PlanRepository
import com.tally.core.BudgetPeriod
import com.tally.core.PaceReading
import com.tally.core.TxType
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.temporal.ChronoUnit
import javax.inject.Inject
import kotlin.math.roundToInt
import kotlin.math.roundToLong

/** How many periods the Trend lens reads, the selected one included. */
internal const val TREND_PERIODS = 6

/** How many rows a breakdown shows before the rest fold into one. */
internal const val TOP_CATEGORIES = 8

/** How many of the period's largest expenses are listed. */
internal const val LARGEST_COUNT = 3

/** The three ways Insights reads the money. */
enum class InsightsLens(val label: String) {
    MONTH("Month"),
    TREND("Trend"),
    CALENDAR("Calendar"),
    WORTH("Worth"),
}

/** How many payees the Month lens lists. */
internal const val TOP_PAYEES = 5

/**
 * One row of a breakdown: a category, the uncategorized pool ([categoryId] null, [folded] 0), or
 * every category past the top rows folded together ([folded] > 0).
 */
@Immutable
data class CategorySlice(
    val key: String,
    val categoryId: Long?,
    val name: String,
    val icon: String?,
    /** Palette index; null draws the slate hue. */
    val color: Int?,
    val total: Long,
    val count: Int,
    val folded: Int = 0,
) {
    val isFolded: Boolean get() = folded > 0
    val isUncategorized: Boolean get() = categoryId == null && folded == 0
}

/** One period of the Trend lens. [offset] is periods from the current one (0 now, -1 the one before). */
@Immutable
data class TrendPoint(
    val period: BudgetPeriod,
    val offset: Int,
    val spent: Long,
    val income: Long,
) {
    val net: Long get() = income - spent
    /** The current period, still running: its totals are partial. */
    val running: Boolean get() = offset == 0
}

/**
 * The Trend lens's readings. Averages are over the periods that have any entry, leaving out the
 * period still running so a half month never drags the average down; [highest] and [lowest] are
 * indices into the points.
 */
@Immutable
data class TrendSummary(
    val averageSpent: Long = 0,
    val averageIncome: Long = 0,
    val months: Int = 0,
    val highest: Int? = null,
    val lowest: Int? = null,
    /** Periods whose spending passed the overall budget; 0 without a budget. */
    val overBudget: Int = 0,
    /** The averages lean on the running period alone, because no full one has entries. */
    val partial: Boolean = false,
) {
    val averageNet: Long get() = averageIncome - averageSpent
}

/** What one weekday took over the days of the period so far. */
@Immutable
data class WeekdaySpend(val day: DayOfWeek, val total: Long, val days: Int) {
    val average: Long get() = if (days > 0) total / days else 0L
}

/** The Calendar lens's readings over the days of the period so far. */
@Immutable
data class CalendarSummary(
    val daysSoFar: Int = 0,
    val activeDays: Int = 0,
    val quietDays: Int = 0,
    val longestQuietRun: Int = 0,
    val perActiveDay: Long = 0,
    val total: Long = 0,
    val busiestDate: LocalDate? = null,
    val busiestTotal: Long = 0,
    /** The weekday with the highest average spend a day. */
    val topWeekday: DayOfWeek? = null,
    val topWeekdayTotal: Long = 0,
    /** How many of [topWeekday] fell in the days so far. */
    val topWeekdayDays: Int = 0,
    val weekendTotal: Long = 0,
    /** Every weekday in week order (the owner's first day first), with what it took. */
    val weekdays: List<WeekdaySpend> = emptyList(),
)

@Immutable
data class InsightsState(
    val today: LocalDate,
    val period: BudgetPeriod,
    /** 0 is the current period, -1 the one before. Never ahead of today. */
    val offset: Int = 0,
    val weekStartsMonday: Boolean = true,
    /** Days of the period read so far: today included in the current one, every day of a past one. */
    val elapsedDays: Int = period.elapsedDays(today),
    /** Spent on each day of the period, first day first. */
    val dayTotals: List<Long> = List(period.days) { 0L },
    /** Running spend at the end of each day so far; days still ahead are not in it. */
    val cumulative: List<Long> = List(elapsedDays) { 0L },
    /** The heat step (0..4) of each day of the period; days still ahead are 0. */
    val heat: List<Int> = List(period.days) { 0 },
    val spent: Long = 0,
    val income: Long = 0,
    val expenseCount: Int = 0,
    /** The overall monthly budget, null when none is set. */
    val budget: Long? = null,
    val reading: PaceReading? = null,
    /** What the previous period had spent by the same day (all of it, for a past period). */
    val lastPeriodSameDay: Long = 0,
    val categories: List<CategorySlice> = emptyList(),
    /** Distinct spending categories before the fold. */
    val categoryCount: Int = 0,
    val incomeSources: List<CategorySlice> = emptyList(),
    val largest: List<TransactionRow> = emptyList(),
    /** What the [largest] entries add up to. */
    val largestTotal: Long = 0,
    /** Every entry of the period by day, newest first within a day. */
    val entriesByDay: Map<LocalDate, List<TransactionRow>> = emptyMap(),
    val calendar: CalendarSummary = CalendarSummary(),
    /** The trend periods, oldest first, ending at [period]. */
    val trend: List<TrendPoint> = emptyList(),
    val trendSummary: TrendSummary = TrendSummary(),
    /** The period's biggest payees by what they took, from the entries' notes. */
    val payees: List<PayeeTotal> = emptyList(),
    val worth: WorthReading = WorthReading(),
    val loaded: Boolean = false,
) {
    val isCurrentPeriod: Boolean get() = offset >= 0
    val daysLeft: Int get() = if (isCurrentPeriod) period.daysLeft(today) else 0
    val perDay: Long get() = if (elapsedDays > 0) spent / elapsedDays else 0L
    val net: Long get() = income - spent

    /** The share of the period's income not spent; null with no income. */
    val savingsRate: Int? get() = keptShare(income, spent)

    /** Where an even spend of today's rate would end the period; null once the period is over or before any spend. */
    val projected: Long?
        get() = if (isCurrentPeriod && elapsedDays in 1 until period.days && spent > 0) {
            spent * period.days / elapsedDays
        } else null
}

// ── Pure readings, held by InsightsLogicTest ───────────────────────────────────────────────────

/** Spent on each day of [period], first day first. Totals dated outside the period are ignored. */
internal fun dailySeries(dayTotals: List<DayTotal>, period: BudgetPeriod): List<Long> {
    val out = LongArray(period.days)
    dayTotals.forEach { d ->
        if (d.date in period) {
            out[ChronoUnit.DAYS.between(period.start, d.date).toInt()] += d.total
        }
    }
    return out.toList()
}

/**
 * The running spend at the end of each day of [period] up to [today] (every day of a past
 * period, none of one not begun). Days still ahead are left out, so the line stops at today.
 */
internal fun cumulative(dayTotals: List<DayTotal>, period: BudgetPeriod, today: LocalDate): List<Long> {
    val daily = dailySeries(dayTotals, period)
    return daily.take(period.elapsedDays(today)).runningReduce { acc, v -> acc + v }
}

/**
 * Where the previous period's reading stops: by the same day for the current period (so "vs last
 * month" compares like with like), its whole length for a past one.
 */
internal fun previousPeriodEnd(period: BudgetPeriod, offset: Int, today: LocalDate): LocalDate {
    val previous = period.shift(-1)
    if (offset < 0) return previous.endExclusive
    val sameDay = previous.start.plusDays(period.elapsedDays(today).toLong())
    return if (sameDay.isBefore(previous.endExclusive)) sameDay else previous.endExclusive
}

/** What an even spend of [budget] would have used by the end of day [dayIndex] (0 is the first day). */
internal fun paceAt(budget: Long, dayIndex: Int, days: Int): Long {
    if (budget <= 0L || days <= 0) return 0L
    val elapsed = (dayIndex + 1).coerceIn(0, days)
    return (budget.toDouble() * elapsed / days).roundToLong()
}

/**
 * A day's heat step against the busiest day: 0 unlit, then four steps by quarter. Any spend at all
 * lights the first step, so a coffee never reads as an empty day.
 */
internal fun heatStep(value: Long, max: Long): Int {
    if (value <= 0L || max <= 0L) return 0
    val f = value.toDouble() / max
    return when {
        f <= 0.25 -> 1
        f <= 0.5 -> 2
        f <= 0.75 -> 3
        else -> 4
    }
}

/** The day of a [days]-long line under a pointer at [x] of [width], clamped to the [drawn] days. */
internal fun dayIndexAt(x: Float, width: Int, days: Int, drawn: Int, rtl: Boolean): Int {
    if (drawn <= 0 || width <= 0 || days <= 1) return 0
    val along = (x / width).coerceIn(0f, 1f)
    val f = if (rtl) 1f - along else along
    return (f * (days - 1)).roundToInt().coerceIn(0, drawn - 1)
}

/**
 * Category totals as ranked rows, largest first. Entries with no category (or one that no longer
 * resolves) pool as "Uncategorized" in the slate hue. Past [limit] rows the tail folds into one
 * "Everything else" row, so the list never grows past [limit].
 */
internal fun rankSlices(
    totals: List<CategoryTotal>,
    categories: Map<Long, CategoryEntity>,
    limit: Int = TOP_CATEGORIES,
): List<CategorySlice> {
    val slices = totals
        .filter { it.total > 0L }
        .groupBy { t -> t.categoryId?.takeIf { categories.containsKey(it) } }
        .map { (id, group) ->
            val c = id?.let { categories[it] }
            CategorySlice(
                key = if (id == null) "none" else "c$id",
                categoryId = id,
                name = c?.name ?: "Uncategorized",
                icon = c?.icon,
                color = c?.color,
                total = group.sumOf { it.total },
                count = group.sumOf { it.count },
            )
        }
        .sortedWith(compareByDescending<CategorySlice> { it.total }.thenBy { it.name })
    if (slices.size <= limit || limit < 2) return slices
    val head = slices.take(limit - 1)
    val rest = slices.drop(limit - 1)
    return head + CategorySlice(
        key = "rest",
        categoryId = null,
        name = "Everything else",
        icon = null,
        color = null,
        total = rest.sumOf { it.total },
        count = rest.sumOf { it.count },
        folded = rest.size,
    )
}

/** One trend period from its type totals. */
internal fun trendPoint(period: BudgetPeriod, offset: Int, totals: List<TypeTotal>): TrendPoint = TrendPoint(
    period = period,
    offset = offset,
    spent = totals.firstOrNull { it.type == TxType.EXPENSE }?.total ?: 0L,
    income = totals.firstOrNull { it.type == TxType.INCOME }?.total ?: 0L,
)

/**
 * Averages over the periods with any entry, the running one left out unless it is the only one;
 * the highest month over every period that spent; the lowest only when two or more full periods
 * make it a different reading from the highest.
 */
internal fun summarizeTrend(points: List<TrendPoint>, budget: Long?): TrendSummary {
    val withData = points.indices.filter { points[it].spent > 0L || points[it].income > 0L }
    val basis = withData.filter { !points[it].running }.ifEmpty { withData }
    if (basis.isEmpty()) return TrendSummary()
    val highest = withData.filter { points[it].spent > 0L }.maxByOrNull { points[it].spent }
    val lowest = if (basis.size > 1) basis.minByOrNull { points[it].spent } else null
    return TrendSummary(
        averageSpent = basis.sumOf { points[it].spent } / basis.size,
        averageIncome = basis.sumOf { points[it].income } / basis.size,
        months = basis.size,
        highest = highest,
        lowest = lowest,
        overBudget = if (budget != null && budget > 0L) points.count { it.spent > budget } else 0,
        partial = basis.all { points[it].running },
    )
}

private val WEEKEND = setOf(DayOfWeek.SATURDAY, DayOfWeek.SUNDAY)

/** The calendar's readings over the first [elapsed] days of [daily]. */
internal fun summarizeCalendar(
    daily: List<Long>,
    period: BudgetPeriod,
    elapsed: Int,
    weekStartsMonday: Boolean = true,
): CalendarSummary {
    val days = daily.take(elapsed.coerceIn(0, daily.size))
    if (days.isEmpty()) return CalendarSummary()
    val active = days.count { it > 0L }
    var longest = 0
    var run = 0
    days.forEach { v ->
        if (v > 0L) {
            run = 0
        } else {
            run++
            if (run > longest) longest = run
        }
    }
    val total = days.sum()
    val busiest = days.indices.maxByOrNull { days[it] }?.takeIf { days[it] > 0L }
    val weekdayOf = { i: Int -> period.start.plusDays(i.toLong()).dayOfWeek }
    val byWeekday = days.indices.groupBy(weekdayOf)
    val top = byWeekday
        .map { (dow, idx) -> Triple(dow, idx.sumOf { days[it] }, idx.size) }
        .filter { it.second > 0L }
        // By the average day, so a weekday that came round once more this period does not win on count.
        .maxByOrNull { it.second / it.third }
    return CalendarSummary(
        daysSoFar = days.size,
        activeDays = active,
        quietDays = days.size - active,
        longestQuietRun = longest,
        perActiveDay = if (active > 0) total / active else 0L,
        total = total,
        busiestDate = busiest?.let { period.start.plusDays(it.toLong()) },
        busiestTotal = busiest?.let { days[it] } ?: 0L,
        topWeekday = top?.first,
        topWeekdayTotal = top?.second ?: 0L,
        topWeekdayDays = top?.third ?: 0,
        weekendTotal = days.indices.filter { weekdayOf(it) in WEEKEND }.sumOf { days[it] },
        weekdays = List(7) { (if (weekStartsMonday) DayOfWeek.MONDAY else DayOfWeek.SUNDAY).plus(it.toLong()) }.map { dow ->
            val idx = byWeekday[dow].orEmpty()
            WeekdaySpend(dow, idx.sumOf { days[it] }, idx.size)
        },
    )
}

/** Everything Insights shows, from the raw reads. One place, so the screen never sums or sorts. */
internal fun buildInsights(
    today: LocalDate,
    period: BudgetPeriod,
    offset: Int,
    weekStartsMonday: Boolean,
    dayTotals: List<DayTotal>,
    expenseByCategory: List<CategoryTotal>,
    incomeByCategory: List<CategoryTotal>,
    rows: List<TransactionRow>,
    categories: List<CategoryEntity>,
    budget: Long?,
    lastPeriodSameDay: Long,
    trend: List<TrendPoint>,
    payees: List<PayeeTotal> = emptyList(),
    worth: WorthReading = WorthReading(),
): InsightsState {
    // Today falls after every past period, so this is every day of one and the days so far of the current.
    val elapsed = period.elapsedDays(today)
    val daily = dailySeries(dayTotals, period)
    val spent = daily.sum()
    val byId = categories.associateBy { it.id }
    val expenseSlices = rankSlices(expenseByCategory, byId)
    val busiest = daily.take(elapsed).maxOrNull() ?: 0L
    val positiveBudget = budget?.takeIf { it > 0L }
    val largest = rows.filter { it.type == TxType.EXPENSE }.sortedByDescending { it.amount }.take(LARGEST_COUNT)
    return InsightsState(
        today = today,
        period = period,
        offset = offset,
        weekStartsMonday = weekStartsMonday,
        elapsedDays = elapsed,
        dayTotals = daily,
        cumulative = cumulative(dayTotals, period, today),
        heat = daily.mapIndexed { i, v -> if (i < elapsed) heatStep(v, busiest) else 0 },
        spent = spent,
        income = incomeByCategory.sumOf { it.total },
        expenseCount = expenseByCategory.sumOf { it.count },
        budget = positiveBudget,
        reading = positiveBudget?.let { PaceReading(it, spent, period.days, elapsed) },
        lastPeriodSameDay = lastPeriodSameDay,
        categories = expenseSlices,
        categoryCount = expenseByCategory.filter { it.total > 0L }
            .map { t -> t.categoryId?.takeIf { byId.containsKey(it) } }
            .distinct()
            .size,
        incomeSources = rankSlices(incomeByCategory, byId),
        largest = largest,
        largestTotal = largest.sumOf { it.amount },
        entriesByDay = rows.groupBy { it.date },
        calendar = summarizeCalendar(daily, period, elapsed, weekStartsMonday),
        trend = trend,
        trendSummary = summarizeTrend(trend, positiveBudget),
        payees = payees,
        worth = worth,
        loaded = true,
    )
}

/**
 * Insights: the month as a line against its pace, six months side by side, and the days as a heat
 * grid. Every total, rank and step is worked out here, never in composition.
 */
@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class InsightsViewModel @Inject constructor(
    private val ledger: LedgerRepository,
    private val plan: PlanRepository,
    settings: SettingsRepository,
    private val clock: Clock,
) : ViewModel() {

    /** Re-read on resume, so a phone left on Insights overnight shows the new day. */
    private val today = MutableStateFlow(clock.today())

    /** 0 is the current period, -1 the one before. Never ahead of today. */
    private val periodOffset = MutableStateFlow(0)

    fun onResume() { today.value = clock.today() }
    fun previousPeriod() { periodOffset.update { it - 1 } }
    fun nextPeriod() { periodOffset.update { (it + 1).coerceAtMost(0) } }
    fun showPeriod(offset: Int) { periodOffset.value = offset.coerceAtMost(0) }

    private data class Inputs(val current: BudgetPeriod, val offset: Int, val day: LocalDate, val monday: Boolean)

    /** The raw reads of the selected period, carried to the final assembly. */
    private data class PeriodReads(
        val days: List<DayTotal>,
        val spentBy: List<CategoryTotal>,
        val incomeBy: List<CategoryTotal>,
        val rows: List<TransactionRow>,
        val categories: List<CategoryEntity>,
    )

    private data class Frame(val budget: Long?, val lastSameDay: Long, val trend: List<TrendPoint>, val payees: List<PayeeTotal>)

    private val inputs = combine(settings.settings, today, periodOffset) { s, d, o ->
        Inputs(s.periodFor(d), o, d, s.weekStartsMonday)
    }.distinctUntilChanged()

    val state: StateFlow<InsightsState> = inputs.flatMapLatest { i ->
        val period = i.current.shift(i.offset.toLong())
        val previous = period.shift(-1)
        val previousEnd = previousPeriodEnd(period, i.offset, i.day)

        val reads = combine(
            ledger.dailyExpense(period.start, period.endExclusive),
            ledger.byCategory(TxType.EXPENSE, period.start, period.endExclusive),
            ledger.byCategory(TxType.INCOME, period.start, period.endExclusive),
            ledger.between(period),
            ledger.categories(),
        ) { days, spentBy, incomeBy, rows, cats -> PeriodReads(days, spentBy, incomeBy, rows, cats) }

        val trendOffsets = (i.offset - TREND_PERIODS + 1)..i.offset
        val trendFlows: List<Flow<TrendPoint>> = trendOffsets.map { o ->
            val p = i.current.shift(o.toLong())
            ledger.typeTotals(p.start, p.endExclusive).map { t -> trendPoint(p, o, t) }
        }
        val trend = combine(trendFlows) { points -> points.toList() }

        val frame = combine(
            plan.budgets(),
            ledger.typeTotals(previous.start, previousEnd),
            trend,
            ledger.payees(TxType.EXPENSE, period.start, period.endExclusive, TOP_PAYEES),
        ) { budgets, last, points, payees ->
            Frame(
                budget = budgets.firstOrNull { it.categoryId == BudgetEntity.OVERALL }?.amount,
                lastSameDay = last.firstOrNull { it.type == TxType.EXPENSE }?.total ?: 0L,
                trend = points,
                payees = payees,
            )
        }

        // Net worth rebuilt from every entry: read once per change, not per lens.
        val worth = combine(ledger.balances(), ledger.allValues(), ledger.flows()) { accounts, values, flows ->
            worthReading(accounts, values, flows, i.current, i.offset, i.day)
        }

        combine(reads, frame, worth) { r, c, w ->
            buildInsights(
                today = i.day,
                period = period,
                offset = i.offset,
                weekStartsMonday = i.monday,
                dayTotals = r.days,
                expenseByCategory = r.spentBy,
                incomeByCategory = r.incomeBy,
                rows = r.rows,
                categories = r.categories,
                budget = c.budget,
                lastPeriodSameDay = c.lastSameDay,
                trend = c.trend,
                payees = c.payees,
                worth = w,
            )
        }
    }.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5_000),
        InsightsState(today = clock.today(), period = BudgetPeriod.containing(clock.today())),
    )
}
