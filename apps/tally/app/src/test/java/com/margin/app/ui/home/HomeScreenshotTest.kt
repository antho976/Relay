package com.margin.app.ui.home

import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.margin.app.testing.Fixtures
import com.margin.app.testing.shoot
import com.margin.core.PaceReading
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class HomeScreenshotTest {

    @get:Rule val compose = createComposeRule()

    private val period = Fixtures.PERIOD
    private val elapsed = period.elapsedDays(Fixtures.TODAY)

    private val populated = HomeState(
        today = Fixtures.TODAY,
        period = period,
        income = 430_000,
        spent = 128_450,
        reading = PaceReading(290_000, 128_450, period.days, elapsed),
        envelopes = listOf(
            Envelope(2, "Dining", "dining", 6, PaceReading(32_000, 21_800, period.days, elapsed)),
            Envelope(1, "Groceries", "cart", 0, PaceReading(50_000, 23_100, period.days, elapsed)),
            Envelope(3, "Transport", "transport", 2, PaceReading(12_000, 4_100, period.days, elapsed)),
            Envelope(4, "Shopping", "bag", 4, PaceReading(15_000, 19_999, period.days, elapsed)),
        ),
        upcoming = listOf(
            UpcomingBill(1, "Hydro-Québec", 7_840, com.margin.core.TxType.EXPENSE, Fixtures.TODAY.plusDays(4), 4, "bolt", 7, "Chequing", true),
            UpcomingBill(2, "Salary", 215_000, com.margin.core.TxType.INCOME, Fixtures.TODAY.plusDays(2), 2, "work", 0, "Chequing", true),
        ),
        recent = Fixtures.rows.take(5),
        accounts = Fixtures.accounts,
        hasEntries = true,
        loaded = true,
        // Sunday 11 Oct to Saturday 17 Oct; today is the Wednesday.
        week = listOf(4_210L, 9_830L, 2_150L, 3_410L, 0L, 0L, 0L),
        weekStart = Fixtures.TODAY.minusDays(3),
        lastPeriodSameDay = 141_200,
        lastPeriodIncomeSameDay = 215_000,
    )

    private val zero = HomeState(today = Fixtures.TODAY, period = period, loaded = true)

    @Test fun home() = compose.shoot("home") { HomeScreen(populated, HomeActions()) }
    @Test fun home200() = compose.shoot("home-200", fontScale = 2f) { HomeScreen(populated, HomeActions()) }
    @Test fun homeZero() = compose.shoot("home-zero") { HomeScreen(zero, HomeActions()) }
    @Test fun homeOverBudget() = compose.shoot("home-over") {
        HomeScreen(populated.copy(spent = 312_000, reading = PaceReading(290_000, 312_000, period.days, elapsed)), HomeActions())
    }
    @Test fun homeMonochrome() = compose.shoot("home-mono", accentEnabled = false) { HomeScreen(populated, HomeActions()) }
}
