package com.tally.app.ui.entry

import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.data.db.CategoryEntity
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
import com.tally.core.AmountInput
import com.tally.core.CategoryKind
import com.tally.core.TxType
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class EntryScreenshotTest {

    @get:Rule val compose = createComposeRule()

    private val today = Fixtures.TODAY
    private val period = Fixtures.PERIOD

    private fun expenseCategory(id: Long, name: String, color: Int, icon: String) =
        CategoryEntity(id = id, name = name, kind = CategoryKind.EXPENSE, color = color, icon = icon, sortOrder = id.toInt())

    private fun incomeCategory(id: Long, name: String, color: Int, icon: String) =
        CategoryEntity(id = id, name = name, kind = CategoryKind.INCOME, color = color, icon = icon, sortOrder = id.toInt())

    /** The seeded set, so the grid shows its real length and its longest names. */
    private val expenseCategories = listOf(
        expenseCategory(1, "Groceries", 0, "cart"),
        expenseCategory(2, "Dining", 6, "dining"),
        expenseCategory(3, "Transport", 2, "transport"),
        expenseCategory(4, "Housing", 9, "home"),
        expenseCategory(5, "Utilities", 7, "bolt"),
        expenseCategory(6, "Phone & internet", 1, "wifi"),
        expenseCategory(7, "Shopping", 4, "bag"),
        expenseCategory(8, "Health", 5, "health"),
        expenseCategory(9, "Entertainment", 3, "ticket"),
        expenseCategory(10, "Subscriptions", 10, "repeat"),
        expenseCategory(11, "Travel", 8, "flight"),
        expenseCategory(12, "Gifts", 11, "gift"),
        expenseCategory(13, "Other", 10, "dots"),
    )

    private val incomeCategories = listOf(
        incomeCategory(14, "Salary", 0, "work"),
        incomeCategory(15, "Side income", 1, "spark"),
        incomeCategory(16, "Refunds", 2, "refund"),
        incomeCategory(17, "Other income", 10, "dots"),
    )

    private val chequing = Fixtures.accounts[0]
    private val visa = Fixtures.accounts[1]

    private val expense = EntryState(
        draft = EntryDraft(
            type = TxType.EXPENSE,
            amount = AmountInput("42.18"),
            date = today,
            accountId = visa.id,
            categoryId = 1,
            categoryChosen = true,
            note = "Metro",
        ),
        today = today,
        period = period,
        loaded = true,
        categories = expenseCategories,
        accounts = Fixtures.accounts,
        suggestions = listOf("Metro Plus Mont-Royal"),
        category = expenseCategories[0],
        account = visa,
        accountAfter = visa.balance - 4_218,
        month = MonthReading(total = 27_318, count = 9),
        budget = 50_000,
        repeatNext = today.plusMonths(1),
    )

    private val transfer = EntryState(
        draft = EntryDraft(
            type = TxType.TRANSFER,
            amount = AmountInput("900"),
            date = today,
            accountId = chequing.id,
            toAccountId = visa.id,
            note = "Card payment",
        ),
        today = today,
        period = period,
        loaded = true,
        accounts = Fixtures.accounts,
        account = chequing,
        toAccount = visa,
        accountAfter = chequing.balance - 90_000,
        toAccountAfter = visa.balance + 90_000,
        repeatNext = today.plusMonths(1),
    )

    /** Editing a posted salary: the delete and duplicate capsules, money in under the green light. */
    private val editIncome = EntryState(
        draft = EntryDraft(
            id = 4,
            type = TxType.INCOME,
            amount = AmountInput("2150"),
            date = today.minusDays(2),
            accountId = chequing.id,
            categoryId = 14,
            categoryChosen = true,
            note = "Salary",
            recurringId = 1,
        ),
        today = today,
        period = period,
        loaded = true,
        categories = incomeCategories,
        accounts = Fixtures.accounts,
        category = incomeCategories[0],
        account = chequing,
        accountAfter = chequing.balance,
        month = MonthReading(total = 215_000, count = 1),
    )

    /** First run with no account yet: the amount still types, the save waits for an account. */
    private val empty = EntryState(
        draft = EntryDraft(date = today),
        today = today,
        period = period,
        loaded = true,
        categories = expenseCategories,
    )

    @Test fun entryExpense() = compose.shoot("entry-expense") { EntryScreen(expense, EntryActions()) }
    @Test fun entryExpense200() = compose.shoot("entry-expense-200", fontScale = 2f) { EntryScreen(expense, EntryActions()) }
    @Test fun entryTransfer() = compose.shoot("entry-transfer") { EntryScreen(transfer, EntryActions()) }
    @Test fun entryEditIncome() = compose.shoot("entry-edit-income") { EntryScreen(editIncome, EntryActions()) }
    @Test fun entryEmpty() = compose.shoot("entry-empty") { EntryScreen(empty, EntryActions()) }
}
