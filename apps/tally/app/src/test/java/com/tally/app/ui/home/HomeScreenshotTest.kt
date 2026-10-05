package com.tally.app.ui.home

import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
import com.tally.app.ui.plan.GoalLine
import com.tally.core.GoalKind
import com.tally.core.PaceReading
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
        // Sorted as the view model sorts them, so the goldens show the order the app draws.
        envelopes = sortEnvelopes(
            listOf(
                Envelope(2, "Dining", "dining", 6, PaceReading(32_000, 21_800, period.days, elapsed)),
                Envelope(1, "Groceries", "cart", 0, PaceReading(50_000, 23_100, period.days, elapsed)),
                Envelope(3, "Transport", "transport", 2, PaceReading(12_000, 4_100, period.days, elapsed)),
                Envelope(4, "Shopping", "bag", 4, PaceReading(15_000, 19_999, period.days, elapsed)),
            )
        ),
        upcoming = listOf(
            UpcomingBill(1, "Hydro-Québec", 7_840, com.tally.core.TxType.EXPENSE, Fixtures.TODAY.plusDays(4), 4, "bolt", 7, "Chequing", true),
            UpcomingBill(2, "Salary", 215_000, com.tally.core.TxType.INCOME, Fixtures.TODAY.plusDays(2), 2, "work", 0, "Chequing", true),
        ),
        recent = Fixtures.rows.take(5),
        goals = listOf(
            GoalLine(1, "Invest a tenth of pay", 3, saved = 30_000, target = 43_000, targetDate = null, monthsLeft = null, paceFraction = null,
                kind = GoalKind.INVEST, percent = 10, income = 430_000, accountName = "TFSA"),
            GoalLine(2, "Lisbon in May", 5, saved = 140_000, target = 320_000, targetDate = Fixtures.TODAY.plusMonths(7), monthsLeft = 7, paceFraction = 0.42f),
        ),
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

    /**
     * Early in a month the week opens in the last one: Monday 28 Sep to Sunday 4 Oct, today Friday
     * 2 Oct. September's three days read dimmed, Tuesday's included although it ran past October's
     * daily allowance, and the line names what they spent so the week and Out add up.
     */
    @Test fun homeWeekBeforePeriod() = compose.shoot("home-week-before") {
        val today = period.start.plusDays(1)
        HomeScreen(
            populated.copy(
                today = today,
                income = 215_000,
                spent = 6_000,
                reading = PaceReading(290_000, 6_000, period.days, period.elapsedDays(today)),
                week = listOf(3_100L, 12_400L, 2_100L, 4_200L, 1_800L, 0L, 0L),
                weekStart = LocalDate.of(2026, 9, 28),
                lastPeriodSameDay = 9_800,
                lastPeriodIncomeSameDay = 215_000,
            ),
            HomeActions(),
        )
    }

    /** A landscape tablet: two panes under one title. */
    @Config(qualifiers = "w1280dp-h800dp-xhdpi")
    @Test fun homeTablet() = compose.shoot("home-tablet") { HomeScreen(populated, HomeActions()) }

    /** A portrait tablet: one column capped at the reading width, centred. */
    @Config(qualifiers = "w800dp-h1280dp-xhdpi")
    @Test fun homeTabletPortrait() = compose.shoot("home-tablet-portrait") { HomeScreen(populated, HomeActions()) }
}
