package com.tally.app.ui.plan

import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.data.db.BudgetRow
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.CategoryTotal
import com.tally.app.data.db.GoalEntity
import com.tally.app.data.db.GoalWithSaved
import com.tally.app.data.db.RecurringEntity
import com.tally.app.data.db.RecurringRow
import com.tally.app.data.db.TypeTotal
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
import com.tally.core.CategoryKind
import com.tally.core.Frequency
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
class PlanScreenshotTest {

    @get:Rule val compose = createComposeRule()

    private val today = Fixtures.TODAY
    private val period = Fixtures.PERIOD

    private fun category(id: Long, name: String, color: Int, icon: String) =
        CategoryEntity(id = id, name = name, kind = CategoryKind.EXPENSE, color = color, icon = icon, sortOrder = id.toInt())

    private val categories = listOf(
        category(1, "Groceries", 0, "cart"),
        category(2, "Dining", 6, "dining"),
        category(3, "Transport", 2, "transport"),
        category(4, "Housing", 9, "home"),
        category(5, "Utilities", 7, "bolt"),
        category(7, "Shopping", 4, "bag"),
        category(8, "Health", 5, "health"),
        category(9, "Entertainment", 3, "ticket"),
    )

    /** Built through the same fold the ViewModel runs, so the goldens show real readings. */
    private val budgetFold = budgetsReading(
        period = period,
        today = today,
        budgets = listOf(
            BudgetRow(1, 0, 290_000, null, null, null),
            BudgetRow(2, 1, 50_000, "Groceries", 0, "cart"),
            BudgetRow(3, 2, 32_000, "Dining", 6, "dining"),
            BudgetRow(4, 3, 12_000, "Transport", 2, "transport"),
            BudgetRow(5, 7, 15_000, "Shopping", 4, "bag"),
        ),
        spending = listOf(
            CategoryTotal(1, 23_100, 9),
            CategoryTotal(2, 21_800, 11),
            CategoryTotal(3, 4_100, 3),
            CategoryTotal(7, 19_999, 2),
            CategoryTotal(9, 8_450, 4),
            CategoryTotal(8, 3_299, 1),
            CategoryTotal(4, 145_000, 1),
        ),
        totals = listOf(TypeTotal(TxType.EXPENSE, 225_748), TypeTotal(TxType.INCOME, 430_000)),
        lastTotals = listOf(TypeTotal(TxType.EXPENSE, 271_040)),
        categories = categories,
        // Three months before this one: rent every month, the hydro bill every other, a show now and then.
        history = listOf(
            listOf(CategoryTotal(4, 145_000, 1), CategoryTotal(5, 7_840, 1), CategoryTotal(9, 8_000, 2)),
            listOf(CategoryTotal(4, 145_000, 1), CategoryTotal(9, 9_500, 3), CategoryTotal(8, 4_000, 1)),
            listOf(CategoryTotal(4, 145_000, 1), CategoryTotal(5, 7_840, 1)),
        ),
        step = 1_000,
    )

    private fun bill(
        id: Long,
        name: String,
        amount: Long,
        daysAway: Long,
        icon: String?,
        color: Int?,
        type: TxType = TxType.EXPENSE,
        frequency: Frequency = Frequency.MONTHLY,
        interval: Int = 1,
        account: String = "Chequing",
        toAccount: String? = null,
        autoPost: Boolean = true,
        active: Boolean = true,
    ): RecurringRow {
        val next = today.plusDays(daysAway)
        return RecurringRow(
            recurring = RecurringEntity(
                id = id,
                name = name,
                type = type,
                amount = amount,
                accountId = 1,
                toAccountId = if (toAccount != null) 3 else null,
                categoryId = if (type == TxType.TRANSFER) null else id,
                frequency = frequency,
                interval = interval,
                anchorDate = next,
                nextDate = next,
                autoPost = autoPost,
                active = active,
            ),
            accountName = account,
            toAccountName = toAccount,
            categoryName = null,
            categoryColor = color,
            categoryIcon = icon,
        )
    }

    private val billFold = billsReading(
        listOf(
            bill(1, "Rent", 145_000, 17, "home", 9),
            bill(2, "Hydro-Québec", 7_840, 4, "bolt", 7, interval = 2),
            bill(3, "Salary", 215_000, 2, "work", 0, type = TxType.INCOME, frequency = Frequency.WEEKLY, interval = 2),
            bill(4, "Fizz mobile", 4_500, 9, "wifi", 1, account = "Visa", autoPost = false),
            bill(5, "To savings", 20_000, 3, null, null, type = TxType.TRANSFER, toAccount = "Savings"),
            bill(6, "Netflix", 1_899, 22, "repeat", 10, account = "Visa"),
            bill(7, "Car insurance, the long policy with roadside assistance", 118_000, 140, "car", 2, frequency = Frequency.YEARLY),
            bill(8, "Climbing gym", 5_499, -30, "sport", 8, account = "Visa", active = false),
        ),
        today,
    )

    private val goalFold = goalsReading(
        listOf(
            GoalWithSaved(GoalEntity(id = 1, name = "Lisbon in May", target = 320_000, targetDate = LocalDate.of(2027, 5, 1), color = 1), 186_000, LocalDate.of(2026, 7, 2)),
            GoalWithSaved(GoalEntity(id = 2, name = "Emergency cushion", target = 1_200_000, color = 3), 415_000, LocalDate.of(2026, 3, 1)),
            GoalWithSaved(GoalEntity(id = 3, name = "New bike", target = 90_000, targetDate = LocalDate.of(2026, 12, 1), color = 7), 90_000, LocalDate.of(2026, 4, 10)),
        ),
        today,
    )

    private val populated = PlanState(
        today = today,
        period = period,
        loaded = true,
        budgets = budgetFold,
        bills = billFold,
        goals = goalFold,
    )

    private val zero = PlanState(
        today = today,
        period = period,
        loaded = true,
        budgets = BudgetsReading(suggestedCategoryId = 1),
    )

    @Test fun budgets() = compose.shoot("plan-budgets") { PlanScreen(populated, PlanActions(), lens = 0) }
    @Test fun budgets200() = compose.shoot("plan-budgets-200", fontScale = 2f) { PlanScreen(populated, PlanActions(), lens = 0) }
    @Test fun budgetsZero() = compose.shoot("plan-budgets-zero") { PlanScreen(zero, PlanActions(), lens = 0) }
    @Test fun budgetsOver() = compose.shoot("plan-budgets-over") {
        PlanScreen(
            populated.copy(
                budgets = budgetsReading(
                    period, today,
                    budgets = listOf(BudgetRow(1, 0, 200_000, null, null, null)),
                    spending = listOf(CategoryTotal(4, 145_000, 1), CategoryTotal(2, 81_000, 20)),
                    totals = listOf(TypeTotal(TxType.EXPENSE, 226_000)),
                    lastTotals = listOf(TypeTotal(TxType.EXPENSE, 198_000)),
                    categories = categories,
                ),
            ),
            PlanActions(),
            lens = 0,
        )
    }
    @Test fun bills() = compose.shoot("plan-bills") { PlanScreen(populated, PlanActions(), lens = 1) }
    @Test fun bills200() = compose.shoot("plan-bills-200", fontScale = 2f) { PlanScreen(populated, PlanActions(), lens = 1) }
    @Test fun billsZero() = compose.shoot("plan-bills-zero") { PlanScreen(zero, PlanActions(), lens = 1) }
    @Test fun goals() = compose.shoot("plan-goals") { PlanScreen(populated, PlanActions(), lens = 2) }
    @Test fun goals200() = compose.shoot("plan-goals-200", fontScale = 2f) { PlanScreen(populated, PlanActions(), lens = 2) }
    @Test fun goalsZero() = compose.shoot("plan-goals-zero") { PlanScreen(zero, PlanActions(), lens = 2) }
    @Test fun budgetsMonochrome() = compose.shoot("plan-budgets-mono", accentEnabled = false) { PlanScreen(populated, PlanActions(), lens = 0) }
}
