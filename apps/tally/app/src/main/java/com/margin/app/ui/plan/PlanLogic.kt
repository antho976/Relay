package com.margin.app.ui.plan

import androidx.compose.runtime.Immutable
import com.margin.app.data.db.AccountEntity
import com.margin.app.data.db.BudgetEntity
import com.margin.app.data.db.BudgetRow
import com.margin.app.data.db.CategoryEntity
import com.margin.app.data.db.CategoryTotal
import com.margin.app.data.db.ContributionEntity
import com.margin.app.data.db.GoalWithSaved
import com.margin.app.data.db.RecurringEntity
import com.margin.app.data.db.RecurringRow
import com.margin.app.data.db.TypeTotal
import com.margin.core.BudgetPeriod
import com.margin.core.CategoryKind
import com.margin.core.Frequency
import com.margin.core.PaceReading
import com.margin.core.Recurrence
import com.margin.core.TxType
import java.time.LocalDate
import java.time.temporal.ChronoUnit
import kotlin.math.roundToLong

/*
 * The Plan package's pure half: budget envelopes, the bills' monthly readings, goal pace, the
 * editors' drafts and their validation. No Android here, so all of it is unit-tested on the JVM.
 */

/** The longest gap the bill editor's stepper offers between two dates. */
internal const val MAX_INTERVAL = 12

/** Recurrence needs an anchor to exist; its per-month factor does not depend on which. */
private val FACTOR_ANCHOR: LocalDate = LocalDate.of(2000, 1, 1)

// ── Budgets ──────────────────────────────────────────────────────────────────

/** One category budget read against the period. */
@Immutable
data class BudgetLine(
    val categoryId: Long,
    val name: String,
    val icon: String?,
    val color: Int?,
    val reading: PaceReading,
)

/** An expense category that has spending this period and no budget. */
@Immutable
data class UnbudgetedLine(
    val categoryId: Long,
    val name: String,
    val icon: String?,
    val color: Int?,
    val spent: Long,
    val count: Int,
)

/** The Budgets lens, folded once in the ViewModel. */
@Immutable
data class BudgetsReading(
    val spent: Long = 0,
    val lastPeriodSpent: Long = 0,
    /** The overall monthly budget's reading; null when none is set. */
    val overall: PaceReading? = null,
    /** Category budgets, the furthest over pace first. */
    val envelopes: List<BudgetLine> = emptyList(),
    /** What the category budgets add up to. */
    val budgeted: Long = 0,
    val unbudgeted: List<UnbudgetedLine> = emptyList(),
    val unbudgetedSpent: Long = 0,
    /** Where "add one" lands when there are no category budgets yet. */
    val suggestedCategoryId: Long? = null,
)

/** Furthest over pace first, measured against each budget's size so a small envelope can lead. */
internal fun sortEnvelopes(lines: List<BudgetLine>): List<BudgetLine> =
    lines.sortedByDescending { it.reading.paceDelta.toDouble() / it.reading.budget.coerceAtLeast(1) }

/** Expense categories with spending in the period and no budget, biggest spend first. */
internal fun unbudgetedLines(
    spending: List<CategoryTotal>,
    budgeted: Set<Long>,
    categories: List<CategoryEntity>,
): List<UnbudgetedLine> {
    val byId = categories.associateBy { it.id }
    return spending.mapNotNull { total ->
        val id = total.categoryId ?: return@mapNotNull null
        if (total.total <= 0L || id in budgeted) return@mapNotNull null
        val category = byId[id] ?: return@mapNotNull null
        if (category.kind != CategoryKind.EXPENSE) return@mapNotNull null
        UnbudgetedLine(id, category.name, category.icon, category.color, total.total, total.count)
    }.sortedByDescending { it.spent }
}

/** The biggest unbudgeted spend, else the first live expense category without a budget. */
internal fun suggestedBudgetCategory(
    unbudgeted: List<UnbudgetedLine>,
    categories: List<CategoryEntity>,
    budgeted: Set<Long>,
): Long? = unbudgeted.firstOrNull()?.categoryId
    ?: categories.firstOrNull { it.kind == CategoryKind.EXPENSE && !it.archived && it.id !in budgeted }?.id

internal fun budgetsReading(
    period: BudgetPeriod,
    today: LocalDate,
    budgets: List<BudgetRow>,
    spending: List<CategoryTotal>,
    totals: List<TypeTotal>,
    lastTotals: List<TypeTotal>,
    categories: List<CategoryEntity>,
): BudgetsReading {
    val spent = totals.firstOrNull { it.type == TxType.EXPENSE }?.total ?: 0L
    val last = lastTotals.firstOrNull { it.type == TxType.EXPENSE }?.total ?: 0L
    val elapsed = period.elapsedDays(today)
    val spentBy = spending.mapNotNull { t -> t.categoryId?.let { id -> id to t.total } }.toMap()
    val overall = budgets.firstOrNull { it.categoryId == BudgetEntity.OVERALL }
        ?.let { PaceReading(it.amount, spent, period.days, elapsed) }
    val envelopes = sortEnvelopes(
        budgets.filter { it.categoryId != BudgetEntity.OVERALL && it.categoryName != null }.map {
            BudgetLine(
                categoryId = it.categoryId,
                name = it.categoryName.orEmpty(),
                icon = it.categoryIcon,
                color = it.categoryColor,
                reading = PaceReading(it.amount, spentBy[it.categoryId] ?: 0L, period.days, elapsed),
            )
        }
    )
    val budgetedIds = envelopes.map { it.categoryId }.toSet()
    val unbudgeted = unbudgetedLines(spending, budgetedIds, categories)
    return BudgetsReading(
        spent = spent,
        lastPeriodSpent = last,
        overall = overall,
        envelopes = envelopes,
        budgeted = envelopes.sumOf { it.reading.budget },
        unbudgeted = unbudgeted,
        unbudgetedSpent = unbudgeted.sumOf { it.spent },
        suggestedCategoryId = suggestedBudgetCategory(unbudgeted, categories, budgetedIds),
    )
}

/** The mean of the months that had any spending, so a first month on record is not averaged with blanks. */
internal fun averageOnRecord(values: List<Long>): Long? {
    val recorded = values.filter { it > 0L }
    return if (recorded.isEmpty()) null else recorded.sum() / recorded.size
}

internal fun roundUpTo(amount: Long, step: Long): Long =
    if (amount <= 0L || step <= 0L) amount.coerceAtLeast(0L) else ((amount + step - 1) / step) * step

internal fun roundTo(amount: Long, step: Long): Long =
    if (amount <= 0L || step <= 0L) amount.coerceAtLeast(0L) else ((amount + step / 2) / step) * step

internal fun ordinal(n: Int): String {
    val suffix = if (n % 100 in 11..13) "th" else when (n % 10) {
        1 -> "st"
        2 -> "nd"
        3 -> "rd"
        else -> "th"
    }
    return "$n$suffix"
}

/** The budget editor's context line. */
internal fun resetLine(startDay: Int): String = "Resets on the ${ordinal(startDay.coerceAtLeast(1))} of every month"

// ── Bills ────────────────────────────────────────────────────────────────────

/** One bill as the Bills lens draws it. */
@Immutable
data class BillLine(
    val id: Long,
    val name: String,
    val type: TxType,
    val amount: Long,
    val anchorDate: LocalDate,
    val nextDate: LocalDate,
    val daysUntil: Long,
    val frequency: Frequency,
    val interval: Int,
    val accountName: String,
    val toAccountName: String?,
    val icon: String?,
    val color: Int?,
    val autoPost: Boolean,
    val active: Boolean,
    /** What it comes to in an average month. */
    val perMonth: Long,
)

/** One bill's share of the monthly cost, for the stacked bar. */
@Immutable
data class CostShare(val name: String, val perMonth: Long, val color: Int?)

/** What falls due in a window: the money and how many payments make it. */
@Immutable
data class DueSum(val total: Long = 0, val payments: Int = 0)

/** The Bills lens, folded once in the ViewModel. */
@Immutable
data class BillsReading(
    /** Active bills, the soonest first. */
    val active: List<BillLine> = emptyList(),
    val paused: List<BillLine> = emptyList(),
    val monthlyCost: Long = 0,
    val monthlyIncome: Long = 0,
    /** Active expense bills, biggest monthly share first. */
    val costShares: List<CostShare> = emptyList(),
    /** Expense bills due today through six days out. */
    val week: DueSum = DueSum(),
    /** Expense bills due today through 29 days out. */
    val month: DueSum = DueSum(),
    /** Active bills that never post on their own. */
    val reminders: Int = 0,
)

/** "Monthly", "Every 2 weeks", "Yearly". */
internal fun frequencyLabel(frequency: Frequency, interval: Int): String {
    val n = interval.coerceAtLeast(1)
    return when (frequency) {
        Frequency.WEEKLY -> if (n == 1) "Weekly" else "Every $n weeks"
        Frequency.MONTHLY -> if (n == 1) "Monthly" else "Every $n months"
        Frequency.YEARLY -> if (n == 1) "Yearly" else "Every $n years"
    }
}

/** "Every month", "Every 3 weeks": the stepper's reading. */
internal fun everyLabel(frequency: Frequency, interval: Int): String {
    val n = interval.coerceAtLeast(1)
    val unit = when (frequency) {
        Frequency.WEEKLY -> "week"
        Frequency.MONTHLY -> "month"
        Frequency.YEARLY -> "year"
    }
    return if (n == 1) "Every $unit" else "Every $n ${unit}s"
}

/** A bill's amount in an average month, rounded to the minor unit. */
internal fun perMonth(amount: Long, frequency: Frequency, interval: Int): Long {
    if (amount <= 0L) return 0L
    val factor = Recurrence(FACTOR_ANCHOR, frequency, interval.coerceIn(1, 52)).perMonthFactor()
    return (amount * factor).roundToLong()
}

internal fun billLine(row: RecurringRow, today: LocalDate): BillLine {
    val item = row.recurring
    return BillLine(
        id = item.id,
        name = item.name,
        type = item.type,
        amount = item.amount,
        anchorDate = item.anchorDate,
        nextDate = item.nextDate,
        daysUntil = ChronoUnit.DAYS.between(today, item.nextDate),
        frequency = item.frequency,
        interval = item.interval,
        accountName = row.accountName,
        toAccountName = row.toAccountName,
        icon = row.categoryIcon,
        color = row.categoryColor,
        autoPost = item.autoPost,
        active = item.active,
        perMonth = perMonth(item.amount, item.frequency, item.interval),
    )
}

/**
 * Every payment of [bills] from each one's next date through [through], overdue ones included:
 * a reminder left unpaid is still owed.
 */
internal fun dueThrough(bills: List<BillLine>, through: LocalDate): DueSum {
    var total = 0L
    var payments = 0
    bills.forEach { b ->
        if (b.nextDate.isAfter(through)) return@forEach
        val rule = Recurrence(b.anchorDate, b.frequency, b.interval.coerceIn(1, 52))
        val count = rule.between(b.nextDate, through, limit = 60).size.coerceAtLeast(1)
        total += b.amount * count
        payments += count
    }
    return DueSum(total, payments)
}

internal fun billsReading(rows: List<RecurringRow>, today: LocalDate): BillsReading {
    val lines = rows.map { billLine(it, today) }
    val active = lines.filter { it.active }.sortedBy { it.nextDate }
    val paused = lines.filter { !it.active }.sortedBy { it.name.lowercase() }
    val expense = active.filter { it.type == TxType.EXPENSE }
    return BillsReading(
        active = active,
        paused = paused,
        monthlyCost = expense.sumOf { it.perMonth },
        monthlyIncome = active.filter { it.type == TxType.INCOME }.sumOf { it.perMonth },
        costShares = expense.filter { it.perMonth > 0L }
            .sortedByDescending { it.perMonth }
            .map { CostShare(it.name, it.perMonth, it.color) },
        week = dueThrough(expense, today.plusDays(6)),
        month = dueThrough(expense, today.plusDays(29)),
        reminders = active.count { !it.autoPost },
    )
}

/** The bill editor's working copy. [id] 0 is a new bill. */
@Immutable
data class BillDraft(
    val id: Long = 0,
    val name: String = "",
    val type: TxType = TxType.EXPENSE,
    /** What is typed in the amount field. */
    val amountText: String = "",
    /** [amountText] read in the owner's currency; null when it is not an amount. */
    val amount: Long? = null,
    val accountId: Long? = null,
    val toAccountId: Long? = null,
    val categoryId: Long? = null,
    val frequency: Frequency = Frequency.MONTHLY,
    val interval: Int = 1,
    val anchorDate: LocalDate,
    val autoPost: Boolean = true,
    val active: Boolean = true,
)

/** What stops a bill from saving, field by field, in the words the form shows. */
@Immutable
data class BillProblems(
    val name: String? = null,
    val amount: String? = null,
    val account: String? = null,
) {
    val any: Boolean get() = name != null || amount != null || account != null
}

internal fun billProblems(d: BillDraft): BillProblems = BillProblems(
    name = if (d.name.isBlank()) "Name the bill so you can find it later" else null,
    amount = if ((d.amount ?: 0L) <= 0L) "Enter an amount above zero" else null,
    account = when {
        d.accountId == null -> when (d.type) {
            TxType.INCOME -> "Pick the account it is paid into"
            TxType.TRANSFER -> "Pick the account it leaves"
            TxType.EXPENSE -> "Pick the account it comes out of"
        }
        d.type == TxType.TRANSFER && d.toAccountId == null -> "Pick the account it goes to"
        d.type == TxType.TRANSFER && d.toAccountId == d.accountId -> "From and to are the same account. A transfer needs two"
        else -> null
    },
)

/**
 * Fills what the person has not picked yet, and drops picks that no longer fit: the default (or
 * first) live account, a second account for a transfer, and no category of the wrong kind.
 */
internal fun resolveBill(
    d: BillDraft,
    accounts: List<AccountEntity>,
    categories: List<CategoryEntity>,
    defaultAccountId: Long,
): BillDraft {
    val live = accounts.filter { !it.archived }
    val accountId = d.accountId?.takeIf { id -> accounts.any { it.id == id } }
        ?: defaultAccountId.takeIf { id -> live.any { it.id == id } }
        ?: live.firstOrNull()?.id
    val toAccountId = if (d.type == TxType.TRANSFER) {
        d.toAccountId?.takeIf { id -> accounts.any { it.id == id } }
            ?: live.firstOrNull { it.id != accountId }?.id
    } else null
    val kind = if (d.type == TxType.INCOME) CategoryKind.INCOME else CategoryKind.EXPENSE
    val categoryId = if (d.type == TxType.TRANSFER) null else {
        d.categoryId?.takeIf { id -> categories.any { it.id == id && it.kind == kind } }
    }
    return d.copy(accountId = accountId, toAccountId = toAccountId, categoryId = categoryId)
}

/** The accounts a bill can pick: the live ones, plus any archived one it already uses. */
internal fun billAccounts(accounts: List<AccountEntity>, d: BillDraft): List<AccountEntity> =
    accounts.filter { !it.archived || it.id == d.accountId || it.id == d.toAccountId }

/** The categories a bill of [type] can pick: live ones of its kind, plus the one it already uses. */
internal fun billCategories(categories: List<CategoryEntity>, type: TxType, pickedId: Long?): List<CategoryEntity> {
    if (type == TxType.TRANSFER) return emptyList()
    val kind = if (type == TxType.INCOME) CategoryKind.INCOME else CategoryKind.EXPENSE
    return categories.filter { it.kind == kind && (!it.archived || it.id == pickedId) }
}

/**
 * The next [count] dates the bill will post on, as saving it would set them: an unchanged rule
 * keeps its stored next date; a new or changed one starts at its first date from today.
 */
internal fun nextDates(d: BillDraft, stored: RecurringEntity?, today: LocalDate, count: Int = 3): List<LocalDate> {
    val rule = Recurrence(d.anchorDate, d.frequency, d.interval.coerceIn(1, 52))
    val first = if (
        stored != null && stored.anchorDate == d.anchorDate &&
        stored.frequency == d.frequency && stored.interval == d.interval
    ) {
        stored.nextDate
    } else {
        rule.onOrAfter(if (d.anchorDate.isAfter(today)) d.anchorDate else today)
    }
    val out = ArrayList<LocalDate>(count)
    var next = first
    repeat(count) {
        out += next
        next = rule.after(next)
    }
    return out
}

// ── Goals ────────────────────────────────────────────────────────────────────

/** One savings goal as the Goals lens and the goal screen draw it. */
@Immutable
data class GoalLine(
    val id: Long,
    val name: String,
    val color: Int,
    val saved: Long,
    val target: Long,
    val targetDate: LocalDate?,
    /** Whole months to the target date, at least 1 until it passes, then 0. Null without a date. */
    val monthsLeft: Int?,
) {
    val fraction: Float get() = if (target <= 0L) 0f else (saved.toDouble() / target).toFloat().coerceAtLeast(0f)
    val reached: Boolean get() = target > 0L && saved >= target
    val left: Long get() = (target - saved).coerceAtLeast(0L)
}

/** The Goals lens, folded once in the ViewModel. */
@Immutable
data class GoalsReading(
    val goals: List<GoalLine> = emptyList(),
    val saved: Long = 0,
    val target: Long = 0,
    val reached: Int = 0,
    /** The soonest target date still ahead among goals not yet reached. */
    val nextDate: LocalDate? = null,
    val nextDateName: String? = null,
) {
    val fraction: Float get() = if (target <= 0L) 0f else (saved.toDouble() / target).toFloat().coerceAtLeast(0f)
    val left: Long get() = (target - saved).coerceAtLeast(0L)
}

/** Whole months from [today] to [date], rounded up: at least 1 until the date passes, then 0. */
internal fun monthsUntil(today: LocalDate, date: LocalDate): Int {
    if (date.isBefore(today)) return 0
    var months = ChronoUnit.MONTHS.between(today, date).toInt()
    if (today.plusMonths(months.toLong()).isBefore(date)) months++
    return months.coerceAtLeast(1)
}

internal fun goalLine(row: GoalWithSaved, today: LocalDate): GoalLine = GoalLine(
    id = row.goal.id,
    name = row.goal.name,
    color = row.goal.color,
    saved = row.saved,
    target = row.goal.target,
    targetDate = row.goal.targetDate,
    monthsLeft = row.goal.targetDate?.let { monthsUntil(today, it) },
)

internal fun goalsReading(rows: List<GoalWithSaved>, today: LocalDate): GoalsReading {
    val goals = rows.map { goalLine(it, today) }
    val next = goals
        .filter { !it.reached && it.targetDate != null && !it.targetDate.isBefore(today) }
        .minByOrNull { it.targetDate ?: today }
    return GoalsReading(
        goals = goals,
        saved = goals.sumOf { it.saved.coerceAtLeast(0L) },
        target = goals.sumOf { it.target.coerceAtLeast(0L) },
        reached = goals.count { it.reached },
        nextDate = next?.targetDate,
        nextDateName = next?.name,
    )
}

/** How a goal is filling: money in a month on average since its first contribution, and when that lands. */
@Immutable
data class GoalPace(
    val perMonth: Long,
    /** Months since the first contribution, this one included. */
    val months: Int,
    /** When the target is met at [perMonth]; null once reached or with nothing coming in. */
    val reachBy: LocalDate?,
)

internal fun goalPace(contributions: List<ContributionEntity>, saved: Long, target: Long, today: LocalDate): GoalPace? {
    if (contributions.isEmpty() || saved <= 0L) return null
    val first = contributions.minOf { it.date }
    val months = monthsUntil(first, today)
    val perMonth = saved / months
    val left = target - saved
    val reachBy = if (left <= 0L || perMonth <= 0L) null else today.plusMonths((left + perMonth - 1) / perMonth)
    return GoalPace(perMonth, months, reachBy)
}

/** The goal editor's working copy. [id] 0 is a new goal. */
@Immutable
data class GoalDraft(
    val id: Long = 0,
    val name: String = "",
    val targetText: String = "",
    /** [targetText] read in the owner's currency; null when it is not an amount. */
    val target: Long? = null,
    val targetDate: LocalDate? = null,
    val color: Int = 0,
    val archived: Boolean = false,
)

@Immutable
data class GoalProblems(val name: String? = null, val target: String? = null) {
    val any: Boolean get() = name != null || target != null
}

internal fun goalProblems(d: GoalDraft): GoalProblems = GoalProblems(
    name = if (d.name.isBlank()) "Name what you are saving for" else null,
    target = if ((d.target ?: 0L) <= 0L) "Enter a target above zero" else null,
)
