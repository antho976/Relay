package com.tally.app.ui.plan

import androidx.compose.runtime.Immutable
import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.BudgetEntity
import com.tally.app.data.db.BudgetRow
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.CategoryTotal
import com.tally.app.data.db.ContributionEntity
import com.tally.app.data.db.GoalWithSaved
import com.tally.app.data.db.RecurringEntity
import com.tally.app.data.db.RecurringRow
import com.tally.app.data.db.TransferRow
import com.tally.app.data.db.TypeTotal
import com.tally.app.ui.common.Dates
import com.tally.core.AccountType
import com.tally.core.BillSchedule
import com.tally.core.BudgetPeriod
import com.tally.core.CategoryKind
import com.tally.core.Copy
import com.tally.core.Frequency
import com.tally.core.GoalKind
import com.tally.core.MoneyFormatter
import com.tally.core.PaceReading
import com.tally.core.Recurrence
import com.tally.core.TxType
import com.tally.core.nextDueOnSave
import java.time.LocalDate
import java.time.temporal.ChronoUnit
import kotlin.math.roundToInt
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
    /** Where this period's spending lands at its current pace; null too early in the period to say. */
    val projected: Long? = null,
    /** Budgets the last periods' spending suggests for categories that have none, biggest first. */
    val suggestions: List<BudgetSuggestion> = emptyList(),
) {
    val suggestedTotal: Long get() = suggestions.sumOf { it.amount }
}

/** A budget the owner's own spending suggests: the recent average, rounded to a tidy figure. */
@Immutable
data class BudgetSuggestion(
    val categoryId: Long,
    val name: String,
    val icon: String?,
    val color: Int?,
    val amount: Long,
    /** How many of the recent periods had spending in the category. */
    val months: Int,
)

/** The periods a suggestion averages: the three before this one. */
internal const val SUGGEST_PERIODS = 3

/** A suggestion needs spending in at least this many of those periods, so a one-off buys nothing. */
internal const val SUGGEST_MIN_MONTHS = 2

/** The most suggestions the Budgets lens lists. */
internal const val SUGGEST_MAX = 6

/** Days into the period before a projection means anything. */
internal const val PROJECT_AFTER_DAYS = 3

/** Where [spent] lands by the period's end at the pace so far; null in the first days or with nothing spent. */
internal fun projectedSpend(spent: Long, days: Int, elapsed: Int): Long? {
    if (spent <= 0L || elapsed < PROJECT_AFTER_DAYS || days <= 0) return null
    if (elapsed >= days) return spent
    return (spent.toDouble() * days / elapsed).roundToLong()
}

/**
 * Budgets for the expense categories with no budget, each the average of the [SUGGEST_PERIODS]
 * periods before this one (months with no spending left out), rounded to [step].
 */
internal fun budgetSuggestions(
    history: List<List<CategoryTotal>>,
    budgeted: Set<Long>,
    categories: List<CategoryEntity>,
    step: Long,
): List<BudgetSuggestion> {
    val live = categories.filter { it.kind == CategoryKind.EXPENSE && !it.archived && it.id !in budgeted }
    return live.mapNotNull { c ->
        val values = history.map { period -> period.firstOrNull { it.categoryId == c.id }?.total ?: 0L }
        val months = values.count { it > 0L }
        if (months < SUGGEST_MIN_MONTHS) return@mapNotNull null
        val average = averageOnRecord(values) ?: return@mapNotNull null
        val amount = roundUpTo(average, step)
        if (amount <= 0L) null else BudgetSuggestion(c.id, c.name, c.icon, c.color, amount, months)
    }.sortedByDescending { it.amount }.take(SUGGEST_MAX)
}

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
    history: List<List<CategoryTotal>> = emptyList(),
    step: Long = 0L,
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
        projected = projectedSpend(spent, period.days, elapsed),
        suggestions = budgetSuggestions(history, budgetedIds, categories, step),
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
 * Every payment of [bills] from each one's next date through [through]. A next date already
 * behind counts too, as it has not posted yet: the poster catches up a bill that posts itself and
 * moves a reminder on once its day is over, so a date stays behind only until its next run.
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
 * The next [count] dates the bill will post on, as saving it would set them: the same
 * [nextDueOnSave] the repository applies, over the [stored] row and [lastPosted], the latest date
 * already entered against it. So an unchanged rule keeps its stored next date, a new or changed
 * one starts at its first date from today (and after any date it already posted), and a bill
 * turned back on starts from today instead of showing the dates it was paused through. Empty
 * while the draft is paused: a paused bill posts nothing.
 */
internal fun nextDates(
    d: BillDraft,
    stored: RecurringEntity?,
    today: LocalDate,
    count: Int = 3,
    lastPosted: LocalDate? = null,
): List<LocalDate> {
    if (!d.active) return emptyList()
    val rule = Recurrence(d.anchorDate, d.frequency, d.interval.coerceIn(1, 52))
    val schedule = stored?.let {
        BillSchedule(Recurrence(it.anchorDate, it.frequency, it.interval.coerceIn(1, 52)), it.nextDate, it.autoPost, it.active)
    }
    val first = nextDueOnSave(
        rule = rule,
        autoPost = d.autoPost,
        active = d.active,
        stored = schedule,
        today = today,
        lastPosted = lastPosted,
        notBefore = if (stored == null) d.anchorDate else null,
    )
    val out = ArrayList<LocalDate>(count)
    var next = first
    repeat(count) {
        out += next
        next = rule.after(next)
    }
    return out
}

// ── Goals ────────────────────────────────────────────────────────────────────

/** How a goal stands today. */
enum class GoalStatus { REACHED, ON_TRACK, BEHIND, OPEN }

/** One month of a monthly goal: what it asked, what happened, and what came in. */
@Immutable
data class GoalMonth(val start: LocalDate, val achieved: Long, val asked: Long, val income: Long) {
    val met: Boolean get() = asked > 0L && achieved >= asked
}

/**
 * One goal as the Goals lens, Home and the goal screen draw it, whatever its [kind]:
 *
 * - [GoalKind.SAVINGS]: [saved] is what its contributions hold, against [target].
 * - [GoalKind.BALANCE]: [saved] is the balance read now, from [start] (what it read when set)
 *   toward [target] by [targetDate].
 * - [GoalKind.INVEST], [GoalKind.SAVE]: [saved] is what this month moved into investments, or
 *   kept of what came in; [target] is this month's ask, [percent] of [income] or a set amount.
 */
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
    /** Where the meter's pace tick stands (see [goalPaceFraction]); null once reached, or without a date. */
    val paceFraction: Float?,
    val kind: GoalKind = GoalKind.SAVINGS,
    val start: Long = 0L,
    val percent: Int = 0,
    val income: Long = 0L,
    /** The account a balance or invest goal reads; null when it reads every one. */
    val accountName: String? = null,
    /** A monthly goal's earlier months, oldest first. */
    val history: List<GoalMonth> = emptyList(),
) {
    val monthly: Boolean get() = kind == GoalKind.INVEST || kind == GoalKind.SAVE

    /** A balance goal heading down (a card to pay off, a debt to clear) rather than up. */
    private val falling: Boolean get() = kind == GoalKind.BALANCE && target < start

    val fraction: Float
        get() = when {
            kind == GoalKind.BALANCE -> {
                val span = target - start
                if (span == 0L) (if (saved == target) 1f else 0f) else ((saved - start).toDouble() / span).toFloat().coerceAtLeast(0f)
            }
            target <= 0L -> 0f
            else -> (saved.toDouble() / target).toFloat().coerceAtLeast(0f)
        }

    val reached: Boolean
        get() = when {
            kind == GoalKind.BALANCE -> if (falling) saved <= target else saved >= target && target != start
            else -> target > 0L && saved >= target
        }

    val left: Long get() = (if (falling) saved - target else target - saved).coerceAtLeast(0L)

    /** A monthly goal's share of what came in, in whole percent; null with nothing in. */
    val achievedPercent: Int? get() = if (income > 0L) (saved * 100 / income).toInt() else null

    /** Months in a row met, counting back from the last full one. */
    val streak: Int get() = history.asReversed().takeWhile { it.met }.size

    val status: GoalStatus
        get() = when {
            reached -> GoalStatus.REACHED
            monthly && target <= 0L -> GoalStatus.OPEN
            paceFraction == null -> GoalStatus.OPEN
            fraction + 0.005f >= paceFraction -> GoalStatus.ON_TRACK
            else -> GoalStatus.BEHIND
        }
}

/** The Goals lens, folded once in the ViewModel. [saved] and [target] add up the savings pots only. */
@Immutable
data class GoalsReading(
    val goals: List<GoalLine> = emptyList(),
    val saved: Long = 0,
    val target: Long = 0,
    val reached: Int = 0,
    /** The soonest target date still ahead among goals not yet reached. */
    val nextDate: LocalDate? = null,
    val nextDateName: String? = null,
    val onTrack: Int = 0,
    val behind: Int = 0,
    /** How many goals are savings pots, the ones [saved] and [target] add up. */
    val pots: Int = 0,
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

/**
 * Where the pace tick stands on a goal's meter: the share of the days from the [first]
 * contribution to the [targetDate] that have gone by [today], which is how full an even saving
 * would have it now. Null without a first contribution or a date, or when the date is not after
 * the first contribution; past the date it holds at the end.
 */
internal fun goalPaceFraction(first: LocalDate?, targetDate: LocalDate?, today: LocalDate): Float? {
    if (first == null || targetDate == null || !first.isBefore(targetDate)) return null
    val span = ChronoUnit.DAYS.between(first, targetDate)
    val gone = ChronoUnit.DAYS.between(first, today)
    return (gone.toDouble() / span).toFloat().coerceIn(0f, 1f)
}

/**
 * What a goal meter says after its amounts: the share saved and, when it has a tick, where an even
 * pace would stand. "58 percent; an even pace would hold 34 percent by today".
 */
internal fun goalMeterWords(fraction: Float, paceFraction: Float?): String {
    val saved = "${(fraction * 100f).roundToInt()} percent"
    if (paceFraction == null) return saved
    return "$saved; an even pace would hold ${(paceFraction * 100f).roundToInt()} percent by today"
}

/** A savings pot from its contributions. */
internal fun goalLine(row: GoalWithSaved, today: LocalDate): GoalLine {
    val line = GoalLine(
        id = row.goal.id,
        name = row.goal.name,
        color = row.goal.color,
        saved = row.saved,
        target = row.goal.target,
        targetDate = row.goal.targetDate,
        monthsLeft = row.goal.targetDate?.let { monthsUntil(today, it) },
        paceFraction = null,
    )
    // A reached goal has no pace left to keep, so its meter carries no tick.
    return if (line.reached) line else line.copy(paceFraction = goalPaceFraction(row.firstDate, row.goal.targetDate, today))
}

/** Everything the goal readings need besides the goals: the accounts, and the months they are read over. */
@Immutable
data class GoalInputs(
    val today: LocalDate,
    /** The month running now, then the ones before it, newest first. */
    val periods: List<BudgetPeriod>,
    /** Each of [periods]' type totals, in the same order. */
    val totals: List<List<TypeTotal>>,
    val balances: List<AccountBalance> = emptyList(),
    /** Every transfer over [periods]. */
    val transfers: List<TransferRow> = emptyList(),
)

/** The open accounts' net: what you hold less what you owe. */
internal fun netWorth(balances: List<AccountBalance>): Long = balances.filter { !it.archived }.sumOf { it.balance }

/**
 * What a month moved into [into] from the other accounts, less what it moved back out. A move
 * between two of [into] (TFSA to RRSP) is neither.
 */
internal fun movedInto(into: Set<Long>, transfers: List<TransferRow>, period: BudgetPeriod): Long =
    transfers.filter { it.date in period }.sumOf { t ->
        val toIn = t.toAccountId in into
        val fromIn = t.accountId in into
        when {
            toIn && !fromIn -> t.amount
            fromIn && !toIn -> -t.amount
            else -> 0L
        }
    }

private fun List<TypeTotal>.of(type: TxType): Long = firstOrNull { it.type == type }?.total ?: 0L

/** A monthly goal's ask from a month's income: [percent] of it, or [amount] when no share is set. */
internal fun monthlyAsk(percent: Int, amount: Long, income: Long): Long =
    if (percent > 0) income * percent / 100 else amount

/** Any goal, read against [inputs]. A savings pot reads its contributions, as [goalLine] does. */
internal fun goalLineOf(row: GoalWithSaved, inputs: GoalInputs): GoalLine {
    val g = row.goal
    val today = inputs.today
    return when (g.kind) {
        GoalKind.SAVINGS -> goalLine(row, today)
        GoalKind.BALANCE -> {
            val account = g.accountId?.let { id -> inputs.balances.firstOrNull { it.id == id } }
            val now = account?.balance ?: netWorth(inputs.balances)
            val base = GoalLine(
                id = g.id, name = g.name, color = g.color, saved = now, target = g.target, targetDate = g.targetDate,
                monthsLeft = g.targetDate?.let { monthsUntil(today, it) }, paceFraction = null, kind = g.kind,
                start = g.startAmount, accountName = account?.name,
            )
            if (base.reached) base else base.copy(paceFraction = goalPaceFraction(g.startDate, g.targetDate, today))
        }
        GoalKind.INVEST, GoalKind.SAVE -> {
            val into: Set<Long> = if (g.accountId != null) setOf(g.accountId) else {
                inputs.balances.filter { it.type == AccountType.INVESTMENT }.map { it.id }.toSet()
            }
            fun month(i: Int): GoalMonth {
                val p = inputs.periods[i]
                val t = inputs.totals.getOrElse(i) { emptyList() }
                val income = t.of(TxType.INCOME)
                val achieved = if (g.kind == GoalKind.INVEST) movedInto(into, inputs.transfers, p) else income - t.of(TxType.EXPENSE)
                return GoalMonth(p.start, achieved, monthlyAsk(g.percent, g.target, income), income)
            }
            val current = if (inputs.periods.isEmpty()) GoalMonth(today, 0L, monthlyAsk(g.percent, g.target, 0L), 0L) else month(0)
            val period = inputs.periods.firstOrNull()
            GoalLine(
                id = g.id, name = g.name, color = g.color, saved = current.achieved, target = current.asked,
                targetDate = null, monthsLeft = null,
                paceFraction = period?.let { (it.elapsedDays(today).toFloat() / it.days).coerceIn(0f, 1f) },
                kind = g.kind, percent = g.percent, income = current.income,
                accountName = g.accountId?.let { id -> inputs.balances.firstOrNull { it.id == id }?.name },
                history = (1 until inputs.periods.size).map { month(it) }.reversed(),
            ).let { if (it.reached) it.copy(paceFraction = null) else it }
        }
    }
}

/** The lens's figures over [goals]. */
internal fun readGoals(goals: List<GoalLine>, today: LocalDate): GoalsReading {
    val pots = goals.filter { it.kind == GoalKind.SAVINGS }
    val next = goals
        .filter { !it.reached && it.targetDate != null && !it.targetDate.isBefore(today) }
        .minByOrNull { it.targetDate ?: today }
    return GoalsReading(
        goals = goals,
        saved = pots.sumOf { it.saved.coerceAtLeast(0L) },
        target = pots.sumOf { it.target.coerceAtLeast(0L) },
        reached = goals.count { it.reached },
        nextDate = next?.targetDate,
        nextDateName = next?.name,
        onTrack = goals.count { it.status == GoalStatus.ON_TRACK },
        behind = goals.count { it.status == GoalStatus.BEHIND },
        pots = pots.size,
    )
}

/** Savings pots alone, read from their contributions. */
internal fun goalsReading(rows: List<GoalWithSaved>, today: LocalDate): GoalsReading =
    readGoals(rows.map { goalLine(it, today) }, today)

/** Every goal, of every kind. */
internal fun goalsReading(rows: List<GoalWithSaved>, inputs: GoalInputs): GoalsReading =
    readGoals(rows.filter { !it.goal.archived }.map { goalLineOf(it, inputs) }, inputs.today)

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

/**
 * A balance goal's pace: how far the balance has moved a month since the goal was set, and when
 * that lands on the target. Null before a month's worth of reading or with no movement toward it.
 */
internal fun balancePace(line: GoalLine, startDate: LocalDate?, today: LocalDate): GoalPace? {
    if (startDate == null || line.kind != GoalKind.BALANCE) return null
    val months = monthsUntil(startDate, today)
    val toward = if (line.target >= line.start) line.saved - line.start else line.start - line.saved
    if (toward <= 0L) return GoalPace(0L, months, null)
    val perMonth = toward / months
    val reachBy = if (line.reached || perMonth <= 0L) null else today.plusMonths((line.left + perMonth - 1) / perMonth)
    return GoalPace(perMonth, months, reachBy)
}

/** The words a goal's row carries under its name. */
internal fun goalRowLine(g: GoalLine, money: MoneyFormatter, today: LocalDate): String = when (g.kind) {
    GoalKind.SAVINGS -> Copy.goalLine(g.saved, g.target, g.monthsLeft, money) +
        (g.targetDate?.let { " · by " + Dates.short(it, today) } ?: "")
    GoalKind.BALANCE -> {
        val head = when {
            g.reached -> "Reached"
            g.monthsLeft == null -> money.formatWhole(g.left) + " to go"
            g.monthsLeft <= 0 -> money.formatWhole(g.left) + " to go, date passed"
            else -> money.formatWhole((g.left + g.monthsLeft - 1) / g.monthsLeft) + " a month for " + Copy.plural(g.monthsLeft, "month")
        }
        head + " · " + (g.accountName ?: "everything you own") + (g.targetDate?.let { " · by " + Dates.short(it, today) } ?: "")
    }
    GoalKind.INVEST, GoalKind.SAVE -> {
        val verb = if (g.kind == GoalKind.INVEST) "Invested" else "Kept"
        when {
            g.percent > 0 && g.income <= 0L -> "Nothing came in yet this month"
            g.reached -> "$verb " + money.formatWhole(g.saved) + " this month" + (g.achievedPercent?.let { ", $it% of what came in" } ?: "")
            else -> money.formatWhole(g.left) + " to go this month" + (if (g.percent > 0) " for ${g.percent}% of what came in" else "")
        } + (if (g.streak > 1) " · ${g.streak} months in a row" else "")
    }
}

/** A goal row's reading at its end: "$1,860 of $3,200", "12% of 10%". */
internal fun goalReading(g: GoalLine, money: MoneyFormatter): String = when {
    g.monthly && g.percent > 0 -> (g.achievedPercent ?: 0).coerceAtLeast(0).toString() + "% of " + g.percent + "%"
    else -> money.formatWhole(g.saved) + " of " + money.formatWhole(g.target)
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
    val kind: GoalKind = GoalKind.SAVINGS,
    /** The account a balance or invest goal reads; null for every one. */
    val accountId: Long? = null,
    /** A monthly goal's share of income; 0 when it asks a set amount. */
    val percent: Int = 10,
    /** True when a monthly goal asks a share of income rather than a set amount. */
    val byShare: Boolean = true,
)

@Immutable
data class GoalProblems(val name: String? = null, val target: String? = null) {
    val any: Boolean get() = name != null || target != null
}

internal fun goalProblems(d: GoalDraft): GoalProblems = GoalProblems(
    name = if (d.name.isBlank()) {
        when (d.kind) {
            GoalKind.SAVINGS -> "Name what you are saving for"
            else -> "Name the goal so you can find it later"
        }
    } else null,
    target = when {
        d.kind == GoalKind.BALANCE && d.target == null -> "Enter the amount to reach"
        (d.kind == GoalKind.INVEST || d.kind == GoalKind.SAVE) && d.byShare && d.percent !in 1..100 -> "Pick a share from 1 to 100 percent"
        (d.kind == GoalKind.INVEST || d.kind == GoalKind.SAVE) && d.byShare -> null
        d.kind != GoalKind.BALANCE && (d.target ?: 0L) <= 0L -> "Enter a target above zero"
        else -> null
    },
)

/** What a goal kind is called, and the one line under it in the editor's choice. */
internal fun goalKindLabel(kind: GoalKind): String = when (kind) {
    GoalKind.SAVINGS -> "Savings pot"
    GoalKind.BALANCE -> "Reach an amount by a date"
    GoalKind.INVEST -> "Invest every month"
    GoalKind.SAVE -> "Keep part of every month"
}

internal fun goalKindHint(kind: GoalKind): String = when (kind) {
    GoalKind.SAVINGS -> "Set money aside for something, by hand"
    GoalKind.BALANCE -> "An account, or everything you own, at an amount by a date"
    GoalKind.INVEST -> "A share of what comes in moved to your investments"
    GoalKind.SAVE -> "A share of what comes in left over after spending"
}
