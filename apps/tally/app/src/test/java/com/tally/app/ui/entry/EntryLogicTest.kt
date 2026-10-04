package com.tally.app.ui.entry

import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.TransactionEntity
import com.tally.core.AccountType
import com.tally.core.AmountInput
import com.tally.core.BudgetPeriod
import com.tally.core.CategoryKind
import com.tally.core.Frequency
import com.tally.core.TxType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

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

    @Test fun aNewExpenseTakesFromItsAccount() {
        assertEquals(-45_438L, balanceAfter(2, visa.balance, null, draft()))
        assertEquals(chequing.balance, balanceAfter(1, chequing.balance, null, draft()))
    }

    @Test fun newIncomeAddsToItsAccount() {
        assertEquals(304_218L, balanceAfter(1, chequing.balance, null, draft(type = TxType.INCOME, account = 1, category = 4)))
    }

    @Test fun anEditSwapsTheStoredAmountForTheNewOne() {
        val stored = TransactionEntity(id = 7, type = TxType.EXPENSE, amount = 3_000, date = today, accountId = 2, categoryId = 1)
        val edited = draft(amount = "50").copy(id = 7)
        assertEquals(-41_220L + 3_000 - 5_000, balanceAfter(2, -41_220, stored, edited))
        // Moved to another account: the old one gets the money back, the new one pays.
        val moved = edited.copy(accountId = 1)
        assertEquals(-41_220L + 3_000, balanceAfter(2, -41_220, stored, moved))
        assertEquals(300_000L - 5_000, balanceAfter(1, 300_000, stored, moved))
    }

    @Test fun aTransferMovesMoneyBetweenItsAccounts() {
        val t = draft(type = TxType.TRANSFER, amount = "900", account = 1, to = 2, category = null)
        assertEquals(300_000L - 90_000, balanceAfter(1, 300_000, null, t))
        assertEquals(-41_220L + 90_000, balanceAfter(2, -41_220, null, t))
        val same = t.copy(toAccountId = 1)
        assertEquals(300_000L, balanceAfter(1, 300_000, null, same))
    }

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
}
