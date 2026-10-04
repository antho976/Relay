package com.tally.app.ui.activity

import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
import com.tally.core.AccountType
import com.tally.core.PaceReading
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class TransactionsScreenshotTest {

    @get:Rule val compose = createComposeRule()

    private val period = Fixtures.PERIOD
    private val elapsed = period.elapsedDays(Fixtures.TODAY)

    private val dining = listOf(
        Fixtures.row(2, "Café Olimpico", 465, 0, category = "Dining", color = 6, icon = "dining"),
        Fixtures.row(7, "Pho Lien", 2_310, 5, category = "Dining", color = 6, icon = "dining"),
        Fixtures.row(12, "Dinner with Marc, the place on Saint-Viateur", 6_840, 8, category = "Dining", color = 6, icon = "dining", account = "Chequing"),
        Fixtures.row(17, "", 1_250, 8, category = "Dining", color = 6, icon = "dining"),
        Fixtures.row(21, "Lunch", 1_895, 11, category = "Dining", color = 6, icon = "dining"),
    )

    private val category = TransactionsState(
        today = Fixtures.TODAY,
        period = period,
        categoryId = 2,
        name = "Dining",
        icon = "dining",
        color = 6,
        entryCount = 38,
        budget = 32_000,
        reading = PaceReading(32_000, 21_800, period.days, elapsed),
        periodTotal = 21_800,
        periodCount = 9,
        lastPeriodTotal = 29_450,
        lastPeriodSameDay = 17_300,
        days = groupByDay(dining),
        summary = summarize(dining),
        loaded = true,
    )

    private val account = TransactionsState(
        today = Fixtures.TODAY,
        period = period,
        accountId = 2,
        name = "Visa",
        accountType = AccountType.CREDIT,
        entryCount = 61,
        balance = -41_220,
        openingBalance = 0,
        flows = AccountFlows(moneyIn = 90_000, moneyOut = 128_450, inCount = 1, outCount = 23),
        days = groupByDay(Fixtures.rows),
        summary = summarize(Fixtures.rows),
        loaded = true,
    )

    private val zero = TransactionsState(
        today = Fixtures.TODAY,
        period = period,
        categoryId = 12,
        name = "Gifts",
        icon = "gift",
        color = 11,
        loaded = true,
    )

    @Test fun transactionsCategory() = compose.shoot("transactions-category") {
        TransactionsScreen(category, "", TransactionsActions())
    }

    @Test fun transactionsCategory200() = compose.shoot("transactions-category-200", fontScale = 2f) {
        TransactionsScreen(category, "", TransactionsActions())
    }

    @Test fun transactionsAccount() = compose.shoot("transactions-account") {
        TransactionsScreen(account, "", TransactionsActions())
    }

    @Test fun transactionsZero() = compose.shoot("transactions-zero") {
        TransactionsScreen(zero, "", TransactionsActions())
    }
}
