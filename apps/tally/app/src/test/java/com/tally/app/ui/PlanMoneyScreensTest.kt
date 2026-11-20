package com.tally.app.ui

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.data.db.AccountBalance
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
import com.tally.app.ui.accounts.AccountsScreen
import com.tally.app.ui.accounts.buildAccountsState
import com.tally.app.ui.categories.CategoriesScreen
import com.tally.app.ui.categories.CategoriesState
import com.tally.app.ui.categories.CategoryLine
import com.tally.app.ui.categories.KindSummary
import com.tally.app.ui.categories.ShareSegment
import com.tally.app.ui.invest.InvestScreen
import com.tally.app.ui.invest.InvestState
import com.tally.app.ui.plan.BillLine
import com.tally.app.ui.plan.BillsReading
import com.tally.app.ui.plan.BillsScreen
import com.tally.app.ui.plan.CostShare
import com.tally.app.ui.plan.DueSum
import com.tally.app.ui.plan.GoalLine
import com.tally.app.ui.plan.GoalScreen
import com.tally.app.ui.plan.GoalState
import com.tally.app.ui.plan.GoalsReading
import com.tally.app.ui.plan.GoalsScreen
import com.tally.app.ui.plan.PlanState
import com.tally.core.AccountType
import com.tally.core.CategoryKind
import com.tally.core.Frequency
import com.tally.core.GoalKind
import com.tally.core.TxType
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The Plan and Money pages in direction C, drawn from state alone: each must compose and show its reading. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class PlanMoneyScreensTest {

    @get:Rule val compose = createComposeRule()

    private val today = Fixtures.TODAY
    private val period = Fixtures.PERIOD

    private fun bill(id: Long, name: String, amount: Long, days: Long, color: Int, icon: String, auto: Boolean = true) = BillLine(
        id, name, TxType.EXPENSE, amount, today, today.plusDays(days), days, Frequency.MONTHLY, 1, "Desjardins chequing", null, icon, color, auto, true, amount,
    )

    @Test fun bills() {
        val active = listOf(
            bill(1, "Hydro-Québec", 8_640, 5, 7, "bolt"),
            bill(2, "Phone plan", 4_500, 9, 2, "phone"),
            bill(3, "Rent", 115_000, 12, 5, "home", auto = false),
        )
        val s = PlanState(
            today, period, loaded = true,
            bills = BillsReading(
                active = active,
                monthlyCost = 128_140, monthlyIncome = 428_000,
                costShares = listOf(CostShare("Rent", 115_000, 5), CostShare("Hydro-Québec", 8_640, 7), CostShare("Phone plan", 4_500, 2)),
                week = DueSum(8_640, 1), month = DueSum(128_140, 3), reminders = 1,
            ),
        )
        compose.shoot("plan-bills") { BillsScreen(s, edit = {}, add = {}) }
        compose.onAllNodesWithText("Hydro-Québec").onFirst().assertExists()
    }

    private val japan = GoalLine(1, "Trip to Japan", 5, 240_000, 500_000, today.plusMonths(5), 5, .45f)
    private val cushion = GoalLine(2, "Emergency fund", 1, 620_000, 900_000, today.plusMonths(8), 8, .7f)
    private val tfsa = GoalLine(3, "TFSA every month", 0, 30_000, 50_000, null, null, null, kind = GoalKind.INVEST)

    @Test fun goals() {
        val s = PlanState(
            today, period, loaded = true,
            goals = GoalsReading(listOf(japan, cushion, tfsa), saved = 860_000, target = 1_400_000, nextDate = today.plusMonths(5), nextDateName = "Trip to Japan", onTrack = 2, behind = 1, pots = 2),
        )
        compose.shoot("plan-goals") { GoalsScreen(s, open = {}, add = {}) }
        compose.onAllNodesWithText("Trip to Japan").onFirst().assertExists()
    }

    @Test fun goal() {
        compose.shoot("plan-goal") { GoalScreen(GoalState(today, loaded = true, goal = japan), back = {}, edit = {}, contribute = { _, _, _ -> }, deleteContribution = {}) }
        compose.onNodeWithText("Put money in, or take it out").assertExists()
    }

    private val banks = listOf(
        AccountBalance(1, "Desjardins chequing", AccountType.CHEQUING, 240_000, false, 0, 318_450, 88),
        AccountBalance(2, "Desjardins Visa", AccountType.CREDIT, 0, false, 1, -41_220, 214),
        AccountBalance(3, "Wealthsimple Cash", AccountType.CHEQUING, 0, false, 2, 64_000, 31),
        AccountBalance(4, "Cash", AccountType.CASH, 12_000, false, 3, 7_350, 9),
    )

    @Test fun accountsWithBanks() {
        val spent = mapOf(1L to 40_200L, 2L to 121_400L, 3L to 14_400L)
        compose.shoot("money-accounts") { AccountsScreen(buildAccountsState(banks, 2, 342, spent), open = {}, add = {}, transfer = {}) }
        compose.onNodeWithText("Stop importing CSVs").assertExists()
    }

    @Test fun investments() {
        val accounts = listOf(
            AccountBalance(10, "Wealthsimple RRSP", AccountType.INVESTMENT, 0, false, 0, 1_220_600, 6, today.minusDays(3)),
            AccountBalance(11, "Wealthsimple TFSA", AccountType.INVESTMENT, 0, false, 1, 841_200, 4, today.minusDays(3)),
        )
        compose.shoot("money-invest") { InvestScreen(InvestState(today, loaded = true, accounts = accounts), import = {}, account = {}, addAccount = {}, room = {}) }
        compose.onNodeWithText("Investments").assertExists()
    }

    @Test fun budgetEdit() {
        val elapsed = period.elapsedDays(today)
        val s = com.tally.app.ui.plan.BudgetEditState(
            categoryId = 1, today = today, period = period, loaded = true, name = "Groceries", icon = "cart", color = 0, existing = 50_000,
            input = com.tally.core.AmountInput("500"), history = listOf(43_000, 51_200, 45_500, 41_200),
            historyStarts = listOf(today.minusMonths(3), today.minusMonths(2), today.minusMonths(1), today).map { it.withDayOfMonth(1) },
            average = 46_800, monthsOnRecord = 3, reading = com.tally.core.PaceReading(50_000, 41_200, period.days, elapsed), suggestAverage = 46_800, suggestLast = 45_500,
        )
        compose.shoot("plan-budget-edit") { com.tally.app.ui.plan.BudgetEditScreen(s, back = {}, key = {}, fill = {}, save = {}, remove = {}) }
        compose.onNodeWithText("What it spent").assertExists()
    }

    @Test fun goalEdit() {
        val s = com.tally.app.ui.plan.GoalEditState(com.tally.app.ui.plan.GoalDraft(kind = GoalKind.BALANCE), today, loaded = true)
        compose.shoot("plan-goal-edit") {
            com.tally.app.ui.plan.GoalEditScreen(s, back = {}, name = {}, target = {}, date = {}, color = {}, kind = {}, account = {}, percent = {}, byShare = {}, save = {}, delete = {})
        }
        compose.onNodeWithText("Savings pot").assertExists()
    }

    @Test fun accountEdit() {
        val s = com.tally.app.ui.accounts.AccountEditState(
            id = 2, draft = com.tally.app.ui.accounts.AccountDraft(name = "Desjardins Visa", type = AccountType.CREDIT, owe = true),
            loaded = true, stored = true, storedName = "Desjardins Visa", entryCount = 214, today = today,
        )
        compose.shoot("money-account-edit") { com.tally.app.ui.accounts.AccountEditScreen(s, com.tally.app.ui.accounts.AccountEditActions()) }
        compose.onNodeWithText("Credit card").assertExists()
    }

    @Test fun categories() {
        fun line(id: Long, name: String, icon: String, color: Int, entries: Int, total: Long, pct: Int) =
            CategoryLine(id, name, CategoryKind.EXPENSE, icon, color, entries, total, pct, false)
        val lines = listOf(
            line(1, "Shopping", "bag", 3, 4, 64_400, 37),
            line(2, "Groceries", "cart", 0, 9, 41_200, 23),
            line(3, "Dining", "dining", 6, 11, 33_600, 19),
            line(4, "Transport", "transport", 2, 6, 27_600, 16),
            line(5, "Coffee", "coffee", 7, 16, 9_200, 5),
            line(6, "Home", "home", 5, 0, 0, 0),
        )
        val summary = KindSummary(
            active = lines, entries = 46, periodTotal = 176_000, periodEntries = 46,
            segments = lines.filter { it.periodTotal > 0 }.map { ShareSegment(it.name, it.color, it.periodTotal, it.percent / 100f, it.percent) },
            usedThisPeriod = 5, mostUsed = lines[4],
        )
        compose.shoot("money-categories") { CategoriesScreen(CategoriesState(today, period, spending = summary, loaded = true), back = {}, open = {}, edit = { _, _ -> }) }
        compose.onNodeWithText("Groceries").assertExists()
    }
}
