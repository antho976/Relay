package com.tally.app.ui.insights

import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.data.db.AccountBalance
import com.tally.app.data.db.AccountValueEntity
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.CategoryTotal
import com.tally.app.data.db.DayTotal
import com.tally.app.data.db.FlowRow
import com.tally.app.data.db.PayeeTotal
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
import com.tally.core.AccountType
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
class InsightsScreenshotTest {

    @get:Rule val compose = createComposeRule()

    private val today = Fixtures.TODAY
    private val period = Fixtures.PERIOD

    private fun cat(id: Long, name: String, color: Int, icon: String, kind: CategoryKind = CategoryKind.EXPENSE) =
        CategoryEntity(id = id, name = name, kind = kind, color = color, icon = icon)

    private val categories = listOf(
        cat(1, "Groceries", 0, "cart"),
        cat(2, "Dining", 6, "dining"),
        cat(3, "Shopping", 4, "bag"),
        cat(4, "Transport", 2, "transport"),
        cat(5, "Entertainment", 3, "ticket"),
        cat(6, "Utilities", 7, "bolt"),
        cat(7, "Health", 5, "health"),
        cat(8, "Subscriptions", 10, "repeat"),
        cat(9, "Gifts", 11, "gift"),
        cat(10, "Phone & internet", 1, "wifi"),
        cat(20, "Salary", 0, "work", CategoryKind.INCOME),
        cat(21, "Side income", 1, "spark", CategoryKind.INCOME),
    )

    // Eleven spending categories, so the tail folds into "Everything else"; they add up to the days.
    private val spentBy = listOf(
        CategoryTotal(1, 25_440, 7),
        CategoryTotal(2, 21_800, 9),
        CategoryTotal(3, 19_999, 2),
        CategoryTotal(4, 12_400, 4),
        CategoryTotal(5, 9_850, 3),
        CategoryTotal(null, 9_000, 2),
        CategoryTotal(6, 7_840, 1),
        CategoryTotal(7, 6_400, 2),
        CategoryTotal(8, 6_497, 3),
        CategoryTotal(9, 5_120, 1),
        CategoryTotal(10, 4_104, 1),
    )

    private val incomeBy = listOf(CategoryTotal(20, 215_000, 1), CategoryTotal(21, 18_000, 2))

    private val days: List<DayTotal> = listOf<Long>(
        10_601, 4_210, 9_830, 0, 6_150, 36_460, 3_410, 0, 7_840, 19_999, 2_310, 0, 6_400, 21_240,
    ).mapIndexed { i, v -> DayTotal(period.start.plusDays(i.toLong()), v) }.filter { it.total > 0L }

    private val trendPoints: List<TrendPoint> = listOf(
        241_000L to 430_000L,
        268_500L to 430_000L,
        312_400L to 445_000L,
        255_800L to 430_000L,
        281_000L to 430_000L,
        128_450L to 233_000L,
    ).mapIndexed { i, (spent, income) ->
        val offset = i - (TREND_PERIODS - 1)
        TrendPoint(period.shift(offset.toLong()), offset, spent, income)
    }

    private val populated = buildInsights(
        today = today,
        period = period,
        offset = 0,
        weekStartsMonday = true,
        dayTotals = days,
        expenseByCategory = spentBy,
        incomeByCategory = incomeBy,
        rows = Fixtures.rows,
        categories = categories,
        budget = 290_000,
        lastPeriodSameDay = 141_200,
        trend = trendPoints,
        payees = listOf(
            PayeeTotal("Metro", 25_440, 7),
            PayeeTotal("Pho Lien", 9_240, 4),
            PayeeTotal("Hydro-Québec", 7_840, 1),
        ),
        worth = worthReading(
            accounts = Fixtures.accounts + AccountBalance(5, "TFSA", AccountType.INVESTMENT, 820_000, false, 4, 0, 6),
            values = listOf(AccountValueEntity(1, 5, today.minusDays(3), 905_000)),
            flows = listOf(
                FlowRow(period.shift(-4).start.plusDays(1), TxType.INCOME, 430_000, 1, null),
                FlowRow(period.shift(-3).start.plusDays(1), TxType.INCOME, 445_000, 1, null),
                FlowRow(period.shift(-3).start.plusDays(9), TxType.EXPENSE, 268_500, 2, null),
                FlowRow(period.shift(-2).start.plusDays(1), TxType.INCOME, 430_000, 1, null),
                FlowRow(period.shift(-2).start.plusDays(12), TxType.EXPENSE, 312_400, 2, null),
                FlowRow(period.shift(-1).start.plusDays(2), TxType.TRANSFER, 30_000, 1, 5),
                FlowRow(period.start.plusDays(1), TxType.INCOME, 215_000, 1, null),
                FlowRow(period.start.plusDays(4), TxType.EXPENSE, 128_450, 2, null),
                FlowRow(period.start.plusDays(5), TxType.TRANSFER, 30_000, 1, 5),
            ),
            current = period,
            offset = 0,
            today = today,
        ),
    )

    private val zero = buildInsights(
        today = today,
        period = period,
        offset = 0,
        weekStartsMonday = true,
        dayTotals = emptyList(),
        expenseByCategory = emptyList(),
        incomeByCategory = emptyList(),
        rows = emptyList(),
        categories = categories,
        budget = null,
        lastPeriodSameDay = 0,
        trend = trendPoints.map { it.copy(spent = 0, income = 0) },
    )

    @Test fun month() = compose.shoot("insights-month") {
        InsightsScreen(populated, InsightsLens.MONTH, InsightsActions())
    }

    @Test fun month200() = compose.shoot("insights-month-200", fontScale = 2f) {
        InsightsScreen(populated, InsightsLens.MONTH, InsightsActions())
    }

    @Test fun monthZero() = compose.shoot("insights-month-zero") {
        InsightsScreen(zero, InsightsLens.MONTH, InsightsActions())
    }

    @Test fun trendLens() = compose.shoot("insights-trend") {
        InsightsScreen(populated, InsightsLens.TREND, InsightsActions())
    }

    @Test fun calendarLens() = compose.shoot("insights-calendar") {
        InsightsScreen(populated, InsightsLens.CALENDAR, InsightsActions())
    }

    @Test fun worthLens() = compose.shoot("insights-worth") {
        InsightsScreen(populated, InsightsLens.WORTH, InsightsActions())
    }

    @Test fun worthLens200() = compose.shoot("insights-worth-200", fontScale = 2f) {
        InsightsScreen(populated, InsightsLens.WORTH, InsightsActions())
    }

    @Test fun worthZero() = compose.shoot("insights-worth-zero") {
        InsightsScreen(zero, InsightsLens.WORTH, InsightsActions())
    }
}
