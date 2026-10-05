package com.tally.app.ui.plan

import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.ContributionEntity
import com.tally.app.data.db.GoalEntity
import com.tally.app.data.db.GoalWithSaved
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
import com.tally.core.AccountType
import com.tally.core.AmountInput
import com.tally.core.CategoryKind
import com.tally.core.Frequency
import com.tally.core.PaceReading
import com.tally.core.TxType
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.time.LocalDate

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class PlanEditorsScreenshotTest {

    @get:Rule val compose = createComposeRule()

    private val today = Fixtures.TODAY
    private val period = Fixtures.PERIOD
    private val elapsed = period.elapsedDays(today)

    // ── Budget editor ────────────────────────────────────────────────────────

    private val historyStarts = (3 downTo 0).map { period.shift(-it.toLong()).start }

    private val budgetState = BudgetEditState(
        categoryId = 2,
        today = today,
        period = period,
        loaded = true,
        name = "Dining",
        icon = "dining",
        color = 6,
        existing = 32_000,
        input = AmountInput("350"),
        history = listOf(26_870, 34_210, 30_150, 21_800),
        historyStarts = historyStarts,
        lastMonth = 30_150,
        average = 30_410,
        monthsOnRecord = 3,
        reading = PaceReading(35_000, 21_800, period.days, elapsed),
        suggestLast = 31_000,
        suggestAverage = 30_000,
    )

    private val budgetNew = BudgetEditState(
        categoryId = 0,
        today = today,
        period = period,
        loaded = true,
        historyStarts = historyStarts,
        reading = PaceReading(0, 0, period.days, elapsed),
    )

    @Test fun budgetEdit() = compose.shoot("budget-edit") { BudgetEditScreen(budgetState, BudgetEditActions()) }
    @Test fun budgetEdit200() = compose.shoot("budget-edit-200", fontScale = 2f) { BudgetEditScreen(budgetState, BudgetEditActions()) }
    @Test fun budgetEditZero() = compose.shoot("budget-edit-zero") { BudgetEditScreen(budgetNew, BudgetEditActions()) }

    // ── Bill editor ──────────────────────────────────────────────────────────

    private val accounts = listOf(
        AccountEntity(id = 1, name = "Chequing", type = AccountType.CHEQUING, sortOrder = 0),
        AccountEntity(id = 2, name = "Visa", type = AccountType.CREDIT, sortOrder = 1),
        AccountEntity(id = 3, name = "Savings", type = AccountType.SAVINGS, sortOrder = 2),
        AccountEntity(id = 4, name = "Cash", type = AccountType.CASH, sortOrder = 3),
    )

    private fun category(id: Long, name: String, color: Int, icon: String) =
        CategoryEntity(id = id, name = name, kind = CategoryKind.EXPENSE, color = color, icon = icon, sortOrder = id.toInt())

    private val expenseCategories = listOf(
        category(1, "Groceries", 0, "cart"),
        category(2, "Dining", 6, "dining"),
        category(3, "Transport", 2, "transport"),
        category(4, "Housing", 9, "home"),
        category(5, "Utilities", 7, "bolt"),
        category(6, "Phone & internet", 1, "wifi"),
        category(10, "Subscriptions", 10, "repeat"),
        category(13, "Other", 10, "dots"),
    )

    private val hydro = BillDraft(
        id = 2,
        name = "Hydro-Québec",
        type = TxType.EXPENSE,
        amountText = "78.40",
        amount = 7_840,
        accountId = 1,
        categoryId = 5,
        frequency = Frequency.MONTHLY,
        interval = 2,
        anchorDate = LocalDate.of(2026, 8, 18),
    )

    private val billState = BillEditState(
        draft = hydro,
        today = today,
        loaded = true,
        stored = true,
        accounts = accounts,
        categories = expenseCategories,
        accountName = "Chequing",
        category = expenseCategories[4],
        preview = nextDates(hydro, null, today),
        perMonth = perMonth(7_840, Frequency.MONTHLY, 2),
        problems = billProblems(hydro),
    )

    private val billBlank = BillDraft(anchorDate = today, accountId = 1)

    private val billNew = BillEditState(
        draft = billBlank,
        today = today,
        loaded = true,
        accounts = accounts,
        accountName = "Chequing",
        categories = expenseCategories,
        preview = nextDates(billBlank, null, today),
        problems = billProblems(billBlank),
        showErrors = true,
    )

    @Test fun billEdit() = compose.shoot("bill-edit") { BillEditScreen(billState, BillEditActions()) }
    @Test fun billEdit200() = compose.shoot("bill-edit-200", fontScale = 2f) { BillEditScreen(billState, BillEditActions()) }
    @Test fun billEditErrors() = compose.shoot("bill-edit-errors") { BillEditScreen(billNew, BillEditActions()) }

    // ── Goal ─────────────────────────────────────────────────────────────────

    private val contributions = listOf(
        ContributionEntity(id = 5, goalId = 1, amount = 40_000, date = today.minusDays(2), note = "October pay"),
        ContributionEntity(id = 4, goalId = 1, amount = -12_000, date = LocalDate.of(2026, 9, 21), note = "Flight deposit moved to Visa"),
        ContributionEntity(id = 3, goalId = 1, amount = 40_000, date = LocalDate.of(2026, 9, 15)),
        ContributionEntity(id = 2, goalId = 1, amount = 58_000, date = LocalDate.of(2026, 8, 15), note = "Tax refund"),
        ContributionEntity(id = 1, goalId = 1, amount = 60_000, date = LocalDate.of(2026, 7, 2), note = "Opening amount"),
    )

    private val lisbon = goalLine(
        GoalWithSaved(GoalEntity(id = 1, name = "Lisbon in May", target = 320_000, targetDate = LocalDate.of(2027, 5, 1), color = 1), 186_000, LocalDate.of(2026, 7, 2)),
        today,
    )

    private val goalState = GoalState(
        today = today,
        loaded = true,
        goal = lisbon,
        contributions = contributions,
        pace = goalPace(contributions, lisbon.saved, lisbon.target, today),
        added = 198_000,
        withdrawn = 12_000,
        addedCount = 4,
        withdrawnCount = 1,
    )

    private val goalEmpty = GoalState(
        today = today,
        loaded = true,
        goal = goalLine(GoalWithSaved(GoalEntity(id = 2, name = "Emergency cushion", target = 1_200_000, color = 3), 0), today),
    )

    @Test fun goal() = compose.shoot("goal") { GoalScreen(goalState, GoalActions()) }
    @Test fun goal200() = compose.shoot("goal-200", fontScale = 2f) { GoalScreen(goalState, GoalActions()) }
    @Test fun goalZero() = compose.shoot("goal-zero") { GoalScreen(goalEmpty, GoalActions()) }
    @Test fun goalDeleted() = compose.shoot("goal-deleted") { GoalScreen(GoalState(today = today, loaded = true), GoalActions()) }

    // ── Goal editor ──────────────────────────────────────────────────────────

    private val goalDraft = GoalDraft(
        id = 1,
        name = "Lisbon in May",
        targetText = "3200",
        target = 320_000,
        targetDate = LocalDate.of(2027, 5, 1),
        color = 1,
    )

    private val goalEdit = GoalEditState(
        draft = goalDraft,
        today = today,
        loaded = true,
        stored = true,
        saved = 186_000,
        firstDate = LocalDate.of(2026, 7, 2),
        monthsLeft = monthsUntil(today, LocalDate.of(2027, 5, 1)),
        problems = goalProblems(goalDraft),
    )

    private val goalNew = GoalEditState(
        draft = GoalDraft(color = 7),
        today = today,
        loaded = true,
        problems = goalProblems(GoalDraft()),
    )

    @Test fun goalEditor() = compose.shoot("goal-edit") { GoalEditScreen(goalEdit, GoalEditActions()) }
    @Test fun goalEditor200() = compose.shoot("goal-edit-200", fontScale = 2f) { GoalEditScreen(goalEdit, GoalEditActions()) }
    @Test fun goalEditorNew() = compose.shoot("goal-edit-new") { GoalEditScreen(goalNew, GoalEditActions()) }
}
