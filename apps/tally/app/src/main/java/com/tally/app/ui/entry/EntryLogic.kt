package com.tally.app.ui.entry

import androidx.compose.runtime.Immutable
import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.TransactionEntity
import com.tally.app.ui.common.LineTone
import com.tally.app.ui.common.ToneLine
import com.tally.core.AmountInput
import com.tally.core.BudgetPeriod
import com.tally.core.CategoryKind
import com.tally.core.Frequency
import com.tally.core.MoneyFormatter
import com.tally.core.Recurrence
import com.tally.core.TxType
import java.time.LocalDate

/*
 * The entry editor's pure half: the draft, the keypad, validation and the readings the screen
 * shows beside the amount. No Android here, so all of it is unit-tested on the JVM.
 */

/** One key of the editor's keypad. */
sealed interface KeypadKey {
    data class Digit(val value: Int) : KeypadKey
    data object Decimal : KeypadKey
    data object Backspace : KeypadKey
}

/**
 * The keypad, top row first: 1 2 3 / 4 5 6 / 7 8 9 / separator 0 backspace. A currency without
 * minor units (JPY) has no separator key; its cell stays empty so the grid keeps its columns.
 */
internal fun keypadRows(showDecimal: Boolean): List<List<KeypadKey?>> = listOf(
    listOf(KeypadKey.Digit(1), KeypadKey.Digit(2), KeypadKey.Digit(3)),
    listOf(KeypadKey.Digit(4), KeypadKey.Digit(5), KeypadKey.Digit(6)),
    listOf(KeypadKey.Digit(7), KeypadKey.Digit(8), KeypadKey.Digit(9)),
    listOf(if (showDecimal) KeypadKey.Decimal else null, KeypadKey.Digit(0), KeypadKey.Backspace),
)

/** What one key press does to the typed amount. */
internal fun AmountInput.press(key: KeypadKey): AmountInput = when (key) {
    is KeypadKey.Digit -> digit(key.value)
    KeypadKey.Decimal -> decimal()
    KeypadKey.Backspace -> backspace()
}

/** The editor's working copy. Ids are null until picked; [id] 0 is a new entry. */
@Immutable
data class EntryDraft(
    val id: Long = 0,
    val type: TxType = TxType.EXPENSE,
    val amount: AmountInput = AmountInput(),
    val date: LocalDate,
    val accountId: Long? = null,
    /** The receiving account of a transfer. */
    val toAccountId: Long? = null,
    val categoryId: Long? = null,
    /** True once the person picked the category (or it came with the entry); a note's last category never overrides it. */
    val categoryChosen: Boolean = false,
    val note: String = "",
    /** New entries only: also make a bill that posts this again. */
    val repeat: Boolean = false,
    val frequency: Frequency = Frequency.MONTHLY,
    val recurringId: Long? = null,
)

/** A category's period so far, this entry included. */
@Immutable
data class MonthReading(val total: Long = 0, val count: Int = 0)

/**
 * What stops the save, in the words the screen shows; null when the draft can be written.
 * Expenses and income need a category, a transfer needs two different accounts.
 */
internal fun validate(draft: EntryDraft): String? = when {
    draft.accountId == null -> "Pick an account"
    draft.type == TxType.TRANSFER && draft.toAccountId != null && draft.toAccountId == draft.accountId ->
        "Pick two different accounts"
    draft.amount.minor <= 0L -> "Type an amount"
    draft.type == TxType.TRANSFER && draft.toAccountId == null -> "Pick the account it goes to"
    draft.type != TxType.TRANSFER && draft.categoryId == null -> "Pick a category"
    else -> null
}

internal fun kindOf(type: TxType): CategoryKind? = when (type) {
    TxType.EXPENSE -> CategoryKind.EXPENSE
    TxType.INCOME -> CategoryKind.INCOME
    TxType.TRANSFER -> null
}

/** The grid for [type]: active categories of its kind, plus [keepId] if the entry already wears an archived one. */
internal fun categoriesFor(type: TxType, all: List<CategoryEntity>, keepId: Long?): List<CategoryEntity> {
    val kind = kindOf(type) ?: return emptyList()
    return all.filter { it.kind == kind && (!it.archived || it.id == keepId) }
}

/** Accounts to choose from: the open ones, plus any archived one the entry already uses. */
internal fun accountChoices(all: List<AccountBalance>, draft: EntryDraft): List<AccountBalance> =
    all.filter { !it.archived || it.id == draft.accountId || it.id == draft.toAccountId }

/**
 * The draft as the screen and the save see it: an account filled in when none was picked (the
 * default account, else the first open one), ids that no longer exist dropped, and a category kept
 * only while it belongs to the current type's kind. A transfer keeps its hidden category, so
 * flipping Expense to Transfer and back does not lose the pick.
 */
internal fun resolveDraft(
    draft: EntryDraft,
    kindCategories: List<CategoryEntity>,
    accounts: List<AccountBalance>,
    defaultAccountId: Long,
): EntryDraft {
    val exists = accounts.mapTo(HashSet()) { it.id }
    val open = accounts.filter { !it.archived }.map { it.id }
    val from = draft.accountId?.takeIf { it in exists }
        ?: defaultAccountId.takeIf { it in open }
        ?: open.firstOrNull()
    val to = draft.toAccountId?.takeIf { it in exists }
    val category = if (draft.type == TxType.TRANSFER) {
        draft.categoryId
    } else {
        draft.categoryId?.takeIf { id -> kindCategories.any { it.id == id } }
    }
    val chosen = draft.categoryChosen && category != null
    if (from == draft.accountId && to == draft.toAccountId && category == draft.categoryId && chosen == draft.categoryChosen) {
        return draft
    }
    return draft.copy(accountId = from, toAccountId = to, categoryId = category, categoryChosen = chosen)
}

/**
 * The draft's category over [period] with this entry in it. [total] and [count] are what the
 * ledger holds, which for an edit already include the stored version, so that is taken out first.
 */
internal fun categoryMonth(
    total: Long,
    count: Int,
    original: TransactionEntity?,
    draft: EntryDraft,
    period: BudgetPeriod,
): MonthReading {
    var sum = total
    var n = count
    if (original != null && original.type == draft.type && original.categoryId == draft.categoryId && original.date in period) {
        sum -= original.amount
        n -= 1
    }
    if (draft.amount.minor > 0 && draft.date in period) {
        sum += draft.amount.minor
        n += 1
    }
    return MonthReading(sum.coerceAtLeast(0), n.coerceAtLeast(0))
}

/**
 * Where a new repeat's bill starts. The entry for [date] is written by hand, so the bill's first
 * posting must fall strictly after it; and like every new bill it never back-fills, so it starts
 * no earlier than [today].
 */
internal fun firstRepeatDate(date: LocalDate, frequency: Frequency, today: LocalDate): LocalDate {
    val from = if (today.isAfter(date)) today else date.plusDays(1)
    return Recurrence(date, frequency).onOrAfter(from)
}

internal fun typeLabel(type: TxType): String = when (type) {
    TxType.EXPENSE -> "Expense"
    TxType.INCOME -> "Income"
    TxType.TRANSFER -> "Transfer"
}

/** The bill a repeat creates is named by its note, else its category, else its type. */
internal fun repeatName(note: String, categoryName: String?, type: TxType): String =
    note.trim().ifEmpty { categoryName?.trim().orEmpty().ifEmpty { typeLabel(type) } }

/** The row the save writes. Null only when no account is set, which [validate] already refuses. */
internal fun EntryDraft.toEntity(original: TransactionEntity?, recurringId: Long?): TransactionEntity? {
    val from = accountId ?: return null
    val transfer = type == TxType.TRANSFER
    return TransactionEntity(
        id = id,
        type = type,
        amount = amount.minor,
        date = date,
        accountId = from,
        toAccountId = if (transfer) toAccountId else null,
        categoryId = if (transfer) null else categoryId,
        note = note.trim(),
        recurringId = recurringId,
        createdAt = original?.createdAt ?: 0L,
    )
}

/**
 * The grid's order: the categories used most in the last few months first, the rest in their own
 * order after them. The two-second log starts where the thumb usually goes.
 */
internal fun byUse(categories: List<CategoryEntity>, uses: Map<Long, Int>): List<CategoryEntity> =
    categories.withIndex()
        .sortedWith(compareByDescending<IndexedValue<CategoryEntity>> { uses[it.value.id] ?: 0 }.thenBy { it.index })
        .map { it.value }

/**
 * The period's spending once this draft is saved. [spent] is the period's spending as stored,
 * which for an edit already holds the stored entry, so that is taken out first.
 */
internal fun spentAfter(spent: Long, original: TransactionEntity?, draft: EntryDraft, period: BudgetPeriod): Long {
    var total = spent
    if (original != null && original.type == TxType.EXPENSE && original.date in period) total -= original.amount
    if (draft.type == TxType.EXPENSE && draft.date in period) total += draft.amount.minor
    return total.coerceAtLeast(0)
}

/** What the month's [budget] has left once this draft is saved; below zero once it is past the limit. */
internal fun leftAfter(budget: Long, spent: Long, original: TransactionEntity?, draft: EntryDraft, period: BudgetPeriod): Long =
    budget - spentAfter(spent, original, draft, period)

/**
 * The line under the amount: where this month's spending stands once this is in. Tally tracks
 * spending against the limit, not what an account holds, so no figure here is ever negative:
 * past the limit it says by how much.
 */
internal fun afterLine(type: TxType, leftAfter: Long?, spentAfter: Long?, fmt: MoneyFormatter): ToneLine? = when {
    type == TxType.TRANSFER -> ToneLine("A transfer moves money between your accounts. Budgets do not count it.")
    type == TxType.INCOME -> null
    leftAfter == null -> spentAfter?.let { ToneLine(fmt.formatWhole(it) + " spent this month with this") }
    leftAfter > 0 -> ToneLine(fmt.formatWhole(leftAfter) + " left this month after this")
    leftAfter == 0L -> ToneLine("This reaches this month's limit", LineTone.WARN)
    else -> ToneLine("Over this month's limit by " + fmt.formatWhole(-leftAfter), LineTone.HELD)
}

/**
 * The picked category's month with this entry in it: against its own limit when it has one
 * ("$418 of $600 in Groceries this month, $182 left"), else what it holds and how many entries.
 */
internal fun categoryLine(type: TxType, name: String, month: MonthReading, budget: Long?, fmt: MoneyFormatter): ToneLine {
    if (type == TxType.EXPENSE && budget != null && budget > 0) {
        val head = fmt.formatWhole(month.total) + " of " + fmt.formatWhole(budget) + " in " + name + " this month"
        return when {
            month.total > budget -> ToneLine(head + ", " + fmt.formatWhole(month.total - budget) + " over its limit", LineTone.HELD)
            month.total == budget -> ToneLine("$head, its limit reached", LineTone.WARN)
            else -> ToneLine(head + ", " + fmt.formatWhole(budget - month.total) + " left")
        }
    }
    if (month.count == 0) return ToneLine("Nothing in $name yet this month")
    return ToneLine(fmt.formatWhole(month.total) + " in " + name + " this month · " + com.tally.core.Copy.plural(month.count, "entry", "entries"))
}

/** "Groceries expense saved", the start of every save's notice. */
internal fun savedLine(type: TxType, categoryName: String?): String = when (type) {
    TxType.TRANSFER -> "Transfer saved"
    else -> listOfNotNull(categoryName, typeLabel(type).lowercase()).joinToString(" ").replaceFirstChar { it.uppercase() } + " saved"
}

/** The notice after a save: what was saved, then where the month stands ("$1,575 left this month"). */
internal fun savedNotice(type: TxType, categoryName: String?, leftAfter: Long?, fmt: MoneyFormatter): String {
    val head = savedLine(type, categoryName)
    if (type != TxType.EXPENSE || leftAfter == null) return head
    return head + " · " + when {
        leftAfter > 0 -> fmt.formatWhole(leftAfter) + " left this month"
        leftAfter == 0L -> "this month's limit reached"
        else -> fmt.formatWhole(-leftAfter) + " over this month's limit"
    }
}

/** A bill an entry already belongs to, as the editor names it. */
@Immutable
data class LinkedBill(val id: Long, val frequency: Frequency, val interval: Int, val nextDate: LocalDate, val active: Boolean, val autoPost: Boolean)

/** The line under "Repeats": when it posts next and how, or that it is paused. */
internal fun linkedBillLine(bill: LinkedBill, day: (LocalDate) -> String): String = when {
    !bill.active -> "Paused: nothing posts until you resume it"
    bill.autoPost -> "Posts itself next on " + day(bill.nextDate)
    else -> "Reminds you next on " + day(bill.nextDate)
}

/**
 * Whether saving makes a bill: Repeat is on and the entry is not already one's posting (a link to
 * a bill that was since deleted counts as none, so the entry can repeat again).
 */
internal fun makesBill(draft: EntryDraft, linked: LinkedBill?): Boolean = draft.repeat && linked == null

/**
 * The grid's order for a visit, held from the first reading of the owner's habits so a cell does
 * not move under the thumb when a save-and-next counts one more use.
 */
internal fun heldOrder(held: Map<TxType, Map<Long, Int>>, type: TxType, uses: Map<Long, Int>): Map<TxType, Map<Long, Int>> =
    if (type in held || uses.isEmpty()) held else held + (type to uses)
