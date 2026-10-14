package com.tally.app.ui.entry

import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.TransactionEntity
import com.tally.app.ui.common.LineTone
import com.tally.app.ui.common.ToneLine
import com.tally.core.AccountType
import com.tally.core.AmountInput
import com.tally.core.BudgetPeriod
import com.tally.core.CategoryKind
import com.tally.core.Frequency
import com.tally.core.MoneyFormatter
import com.tally.core.TxType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate
import java.util.Locale

class EntryLogicTest {

    private val today = LocalDate.of(2026, 10, 14)
    private val period = BudgetPeriod.containing(today)

    private val chequing = AccountBalance(1, "Chequing", AccountType.CHEQUING, 0, false, 0, 300_000, 10)
    private val visa = AccountBalance(2, "Visa", AccountType.CREDIT, 0, false, 1, -41_220, 20)
    private val oldCash = AccountBalance(3, "Old cash", AccountType.CASH, 0, true, 2, 0, 1)
    private val accounts = listOf(chequing, visa, oldCash)

    private val groceries = CategoryEntity(id = 1, name = "Groceries", kind = CategoryKind.EXPENSE, color = 0, icon = "cart")
    private val dining = CategoryEntity(id = 2, name = "Dining", kind = CategoryKind.EXPENSE, color = 6, icon = "dining")
    private val retired = CategoryEntity(id = 3, name = "Retired", kind = CategoryKind.EXPENSE, color = 1, icon = "dots", archived = true)
    private val salary = CategoryEntity(id = 4, name = "Salary", kind = CategoryKind.INCOME, color = 0, icon = "work")
    private val categories = listOf(groceries, dining, retired, salary)

    private fun typed(vararg keys: KeypadKey): AmountInput = keys.fold(AmountInput()) { input, key -> input.press(key) }

    private fun draft(
        type: TxType = TxType.EXPENSE,
        amount: String = "42.18",
        account: Long? = 2,
        to: Long? = null,
        category: Long? = 1,
        date: LocalDate = today,
    ) = EntryDraft(
        type = type,
        amount = AmountInput(amount),
        date = date,
        accountId = account,
        toAccountId = to,
        categoryId = category,
        categoryChosen = category != null,
    )

    // ── Keypad ───────────────────────────────────────────────────────────────

    @Test fun keysTypeAnAmountInMinorUnits() {
        val input = typed(KeypadKey.Digit(1), KeypadKey.Digit(2), KeypadKey.Decimal, KeypadKey.Digit(5))
        assertEquals("12.5", input.text)
        assertEquals(1_250L, input.minor)
    }

    @Test fun backspaceTakesTheLastKeyBack() {
        var input = typed(KeypadKey.Digit(1), KeypadKey.Digit(2), KeypadKey.Decimal, KeypadKey.Digit(5))
        input = input.press(KeypadKey.Backspace)
        assertEquals("12.", input.text)
        assertEquals(1_200L, input.minor)
        input = input.press(KeypadKey.Backspace).press(KeypadKey.Backspace)
        assertEquals("1", input.text)
        assertEquals(100L, input.minor)
    }

    @Test fun keypadStopsAtTheCurrencysDecimals() {
        val input = typed(
            KeypadKey.Digit(3), KeypadKey.Decimal, KeypadKey.Digit(9), KeypadKey.Digit(9), KeypadKey.Digit(9),
        )
        assertEquals("3.99", input.text)
        assertEquals(399L, input.minor)
    }

    @Test fun decimalKeyDoesNothingForAWholeUnitCurrency() {
        val yen = listOf(KeypadKey.Digit(5), KeypadKey.Decimal, KeypadKey.Digit(0))
            .fold(AmountInput(fractionDigits = 0)) { input, key -> input.press(key) }
        assertEquals(50L, yen.minor)
    }

    @Test fun keypadRowsReadLikeAPhoneDialler() {
        val rows = keypadRows(showDecimal = true)
        assertEquals(4, rows.size)
        assertTrue(rows.all { it.size == 3 })
        val digits = rows.flatten().filterIsInstance<KeypadKey.Digit>().map { it.value }
        assertEquals(listOf(1, 2, 3, 4, 5, 6, 7, 8, 9, 0), digits)
        assertEquals(listOf(KeypadKey.Decimal, KeypadKey.Digit(0), KeypadKey.Backspace), rows.last())
    }

    @Test fun noDecimalKeyLeavesItsCellEmpty() {
        val last = keypadRows(showDecimal = false).last()
        assertNull(last[0])
        assertEquals(KeypadKey.Digit(0), last[1])
        assertEquals(KeypadKey.Backspace, last[2])
    }

    // ── Validation ───────────────────────────────────────────────────────────

    @Test fun aCompleteExpenseValidates() = assertNull(validate(draft()))

    @Test fun anExpenseNeedsEverything() {
        assertEquals("Pick an account", validate(draft(account = null)))
        assertEquals("Type an amount", validate(draft(amount = "")))
        assertEquals("Type an amount", validate(draft(amount = "0.")))
        assertEquals("Pick a category", validate(draft(category = null)))
    }

    @Test fun incomeNeedsACategoryToo() {
        assertEquals("Pick a category", validate(draft(type = TxType.INCOME, category = null)))
        assertNull(validate(draft(type = TxType.INCOME, category = 4)))
    }

    @Test fun aTransferNeedsTwoDifferentAccountsAndNoCategory() {
        assertNull(validate(draft(type = TxType.TRANSFER, account = 1, to = 2, category = null)))
        assertEquals("Pick the account it goes to", validate(draft(type = TxType.TRANSFER, account = 1, to = null)))
        assertEquals("Pick two different accounts", validate(draft(type = TxType.TRANSFER, account = 1, to = 1)))
        // The clash is named even before an amount, since it is a mistake rather than a gap.
        assertEquals("Pick two different accounts", validate(draft(type = TxType.TRANSFER, amount = "", account = 1, to = 1)))
    }

    // ── Defaults and lists ───────────────────────────────────────────────────

    @Test fun aNewEntryOpensOnTheDefaultAccount() {
        val kind = categoriesFor(TxType.EXPENSE, categories, null)
        assertEquals(2L, resolveDraft(draft(account = null), kind, accounts, defaultAccountId = 2).accountId)
    }

    @Test fun anArchivedOrMissingDefaultFallsBackToTheFirstOpenAccount() {
        val kind = categoriesFor(TxType.EXPENSE, categories, null)
        assertEquals(1L, resolveDraft(draft(account = null), kind, accounts, defaultAccountId = 3).accountId)
        assertEquals(1L, resolveDraft(draft(account = null), kind, accounts, defaultAccountId = 0).accountId)
        assertEquals(1L, resolveDraft(draft(account = 99), kind, accounts, defaultAccountId = 0).accountId)
        assertNull(resolveDraft(draft(account = null), kind, emptyList(), defaultAccountId = 2).accountId)
    }

    @Test fun anEntryKeepsItsArchivedAccount() {
        val kind = categoriesFor(TxType.EXPENSE, categories, null)
        val d = resolveDraft(draft(account = 3), kind, accounts, defaultAccountId = 2)
        assertEquals(3L, d.accountId)
        assertEquals(listOf(1L, 2L, 3L), accountChoices(accounts, d).map { it.id })
        assertEquals(listOf(1L, 2L), accountChoices(accounts, draft(account = 1)).map { it.id })
    }

    @Test fun aMissingReceivingAccountIsDropped() {
        val d = resolveDraft(draft(type = TxType.TRANSFER, account = 1, to = 99, category = null), emptyList(), accounts, 0)
        assertNull(d.toAccountId)
    }

    @Test fun theGridIsTheTypesKindWithoutArchivedCategories() {
        assertEquals(listOf(1L, 2L), categoriesFor(TxType.EXPENSE, categories, null).map { it.id })
        assertEquals(listOf(1L, 2L, 3L), categoriesFor(TxType.EXPENSE, categories, keepId = 3).map { it.id })
        assertEquals(listOf(4L), categoriesFor(TxType.INCOME, categories, null).map { it.id })
        assertTrue(categoriesFor(TxType.TRANSFER, categories, null).isEmpty())
    }

    @Test fun aCategoryOfTheOtherKindDoesNotCarryOver() {
        val asIncome = draft(type = TxType.INCOME, category = 1)
        val d = resolveDraft(asIncome, categoriesFor(TxType.INCOME, categories, 1), accounts, 0)
        assertNull(d.categoryId)
        assertFalse(d.categoryChosen)
        // A transfer keeps the pick hidden, so flipping back to Expense still has it.
        val asTransfer = resolveDraft(draft(type = TxType.TRANSFER, category = 1, to = 1), emptyList(), accounts, 0)
        assertEquals(1L, asTransfer.categoryId)
    }

    // ── Readings ─────────────────────────────────────────────────────────────

    @Test fun theMonthCountsThisEntryOnce() {
        assertEquals(MonthReading(27_318, 9), categoryMonth(23_100, 8, null, draft(), period))
        val stored = TransactionEntity(id = 7, type = TxType.EXPENSE, amount = 3_000, date = today, accountId = 2, categoryId = 1)
        val edited = draft(amount = "50").copy(id = 7)
        assertEquals(MonthReading(23_100 - 3_000 + 5_000, 8), categoryMonth(23_100, 8, stored, edited, period))
    }

    @Test fun anEntryOutsideThePeriodOrWithoutAnAmountAddsNothing() {
        assertEquals(MonthReading(23_100, 8), categoryMonth(23_100, 8, null, draft(date = today.minusMonths(2)), period))
        assertEquals(MonthReading(23_100, 8), categoryMonth(23_100, 8, null, draft(amount = ""), period))
    }

    // ── Repeat ───────────────────────────────────────────────────────────────

    @Test fun aRepeatStartsAfterTheEntryItCameWith() {
        // Logged today: the bill's first posting is next month, never today again.
        assertEquals(LocalDate.of(2026, 11, 14), firstRepeatDate(today, Frequency.MONTHLY, today))
        // Logged ahead: strictly after that date.
        assertEquals(LocalDate.of(2026, 10, 27), firstRepeatDate(LocalDate.of(2026, 10, 20), Frequency.WEEKLY, today))
        assertEquals(LocalDate.of(2027, 10, 14), firstRepeatDate(today, Frequency.YEARLY, today))
    }

    @Test fun aBackdatedRepeatDoesNotBackFill() {
        // Logged for Aug 31: Sep 30 has passed, so the bill starts at Oct 31, on the anchor's day.
        assertEquals(LocalDate.of(2026, 10, 31), firstRepeatDate(LocalDate.of(2026, 8, 31), Frequency.MONTHLY, today))
        // Logged yesterday, weekly: next week, not today.
        assertEquals(LocalDate.of(2026, 10, 20), firstRepeatDate(today.minusDays(1), Frequency.WEEKLY, today))
    }

    @Test fun aRepeatIsNamedByItsNoteThenCategoryThenType() {
        assertEquals("Netflix", repeatName("  Netflix ", "Subscriptions", TxType.EXPENSE))
        assertEquals("Subscriptions", repeatName("", "Subscriptions", TxType.EXPENSE))
        assertEquals("Transfer", repeatName(" ", null, TxType.TRANSFER))
    }

    // ── The written row ──────────────────────────────────────────────────────

    @Test fun theRowCarriesOnlyWhatItsTypeUses() {
        val transfer = draft(type = TxType.TRANSFER, amount = "900", account = 1, to = 2, category = 1)
            .copy(note = " Card payment ")
            .toEntity(null, null)
        assertEquals(null, transfer?.categoryId)
        assertEquals(2L, transfer?.toAccountId)
        assertEquals("Card payment", transfer?.note)
        assertEquals(90_000L, transfer?.amount)

        val expense = draft(to = 1).toEntity(null, recurringId = 5)
        assertEquals(null, expense?.toAccountId)
        assertEquals(1L, expense?.categoryId)
        assertEquals(5L, expense?.recurringId)

        assertNull(draft(account = null).toEntity(null, null))
    }

    @Test fun anEditKeepsWhenItWasFirstLogged() {
        val stored = TransactionEntity(id = 7, type = TxType.EXPENSE, amount = 3_000, date = today, accountId = 2, categoryId = 1, createdAt = 1_700_000_000_000)
        val row = draft().copy(id = 7).toEntity(stored, null)
        assertEquals(7L, row?.id)
        assertEquals(1_700_000_000_000L, row?.createdAt)
    }

    // ── Spending, not balances ───────────────────────────────────────────────

    private val fmt = MoneyFormatter("CAD", Locale.CANADA)

    @Test fun theMonthsSpendingCountsThisExpenseOnce() {
        assertEquals(140_000L + 4_218L, spentAfter(140_000, null, draft(), period))
        val stored = TransactionEntity(id = 7, type = TxType.EXPENSE, amount = 3_000, date = today, accountId = 2, categoryId = 1)
        assertEquals(140_000L - 3_000 + 5_000, spentAfter(140_000, stored, draft(amount = "50").copy(id = 7), period))
        // Income and other months add nothing; spending never reads below zero.
        assertEquals(140_000L, spentAfter(140_000, null, draft(type = TxType.INCOME, category = 4), period))
        assertEquals(0L, spentAfter(0, stored, draft(date = today.minusMonths(2)).copy(id = 7), period))
    }

    @Test fun theAmountsLineSaysTheLimitInWordsNeverANegative() {
        assertEquals(ToneLine("$1,575 left this month after this"), afterLine(TxType.EXPENSE, 157_530, 142_470, fmt))
        assertEquals(ToneLine("This reaches this month's limit", LineTone.WARN), afterLine(TxType.EXPENSE, 0, 300_000, fmt))
        val over = afterLine(TxType.EXPENSE, -3_100, 303_100, fmt)!!
        assertEquals(ToneLine("Over this month's limit by $31", LineTone.HELD), over)
        assertFalse(over.text.contains("-"))
        // Without a budget it is the month's spending; income says nothing; a transfer says it is not spending.
        assertEquals(ToneLine("$1,424 spent this month with this"), afterLine(TxType.EXPENSE, null, 142_400, fmt))
        assertNull(afterLine(TxType.INCOME, null, null, fmt))
        assertTrue(afterLine(TxType.TRANSFER, null, null, fmt)!!.text.contains("not count"))
    }

    @Test fun theCategoryLineReadsAgainstItsOwnLimit() {
        assertEquals(ToneLine("$418 of $600 in Groceries this month, $182 left"), categoryLine(TxType.EXPENSE, "Groceries", MonthReading(41_800, 9), 60_000, fmt))
        assertEquals(ToneLine("$600 of $600 in Groceries this month, its limit reached", LineTone.WARN), categoryLine(TxType.EXPENSE, "Groceries", MonthReading(60_000, 9), 60_000, fmt))
        assertEquals(ToneLine("$631 of $600 in Groceries this month, $31 over its limit", LineTone.HELD), categoryLine(TxType.EXPENSE, "Groceries", MonthReading(63_100, 10), 60_000, fmt))
        assertEquals(ToneLine("$231 in Dining this month · 4 entries"), categoryLine(TxType.EXPENSE, "Dining", MonthReading(23_100, 4), null, fmt))
        assertEquals(ToneLine("Nothing in Salary yet this month"), categoryLine(TxType.INCOME, "Salary", MonthReading(), null, fmt))
    }

    @Test fun theSavedNoticeSaysWhereTheMonthStands() {
        assertEquals("Groceries expense saved · $1,575 left this month", savedNotice(TxType.EXPENSE, "Groceries", 157_530, fmt))
        assertEquals("Groceries expense saved · this month's limit reached", savedNotice(TxType.EXPENSE, "Groceries", 0, fmt))
        assertEquals("Dining expense saved · $31 over this month's limit", savedNotice(TxType.EXPENSE, "Dining", -3_100, fmt))
        assertEquals("Salary income saved", savedNotice(TxType.INCOME, "Salary", null, fmt))
        assertEquals("Transfer saved", savedNotice(TxType.TRANSFER, null, null, fmt))
    }

    @Test fun anEntryCanBeMadeToRepeatAfterItIsSaved() {
        val stored = draft().copy(id = 7, repeat = true)
        // An edit with Repeat on and no bill makes one, as a new entry does.
        assertTrue(makesBill(stored, linked = null))
        assertTrue(makesBill(draft().copy(repeat = true), linked = null))
        // Already a bill's posting: the switch is not offered, and nothing is made twice.
        val bill = LinkedBill(3, Frequency.MONTHLY, 1, today.plusMonths(1), active = true, autoPost = true)
        assertFalse(makesBill(stored.copy(recurringId = 3), bill))
        assertFalse(makesBill(draft(), linked = null))
    }

    @Test fun aLinkedBillSaysWhenItPostsNext() {
        val bill = LinkedBill(3, Frequency.MONTHLY, 1, LocalDate.of(2026, 11, 14), active = true, autoPost = true)
        assertEquals("Posts itself next on 14 Nov", linkedBillLine(bill) { "${it.dayOfMonth} Nov" })
        assertEquals("Reminds you next on 14 Nov", linkedBillLine(bill.copy(autoPost = false)) { "${it.dayOfMonth} Nov" })
        assertEquals("Paused: nothing posts until you resume it", linkedBillLine(bill.copy(active = false)) { "" })
    }

    @Test fun theGridsOrderHoldsForTheVisit() {
        val first = heldOrder(emptyMap(), TxType.EXPENSE, mapOf(1L to 5, 2L to 3))
        // A save-and-next that counts one more Dining does not reorder the grid under the thumb.
        val later = heldOrder(first, TxType.EXPENSE, mapOf(1L to 5, 2L to 9))
        assertEquals(mapOf(1L to 5, 2L to 3), later[TxType.EXPENSE])
        // Each kind holds its own first reading; an empty reading holds nothing yet.
        assertEquals(mapOf(4L to 2), heldOrder(later, TxType.INCOME, mapOf(4L to 2))[TxType.INCOME])
        assertNull(heldOrder(emptyMap(), TxType.EXPENSE, emptyMap())[TxType.EXPENSE])
    }

    @Test fun newLinesKeepTheVoice() {
        val lines = listOfNotNull(
            afterLine(TxType.EXPENSE, -3_100, null, fmt)?.text,
            afterLine(TxType.TRANSFER, null, null, fmt)?.text,
            categoryLine(TxType.EXPENSE, "Groceries", MonthReading(63_100, 10), 60_000, fmt).text,
            savedNotice(TxType.EXPENSE, "Dining", -3_100, fmt),
        )
        lines.forEach { line -> com.tally.core.Copy.banned.forEach { bad -> assertFalse("\"$line\" has \"$bad\"", line.lowercase().contains(bad)) } }
    }
}
