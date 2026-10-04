package com.tally.app.ui.activity

import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
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
class ActivityScreenshotTest {

    @get:Rule val compose = createComposeRule()

    private val period = Fixtures.PERIOD
    private val rows = Fixtures.rows
    private val summary = summarize(rows)

    private val populated = ActivityState(
        today = Fixtures.TODAY,
        period = period,
        days = groupByDay(rows),
        summary = summary,
        perDay = summary.spent / period.elapsedDays(Fixtures.TODAY),
        loaded = true,
    )

    private val zero = ActivityState(today = Fixtures.TODAY, period = period, perDay = 0L, loaded = true)

    private val searchEmpty = ActivityState(today = Fixtures.TODAY, period = period, query = "ikea", loaded = true)

    @Test fun activity() = compose.shoot("activity") { ActivityScreen(populated, "", ActivityActions()) }

    @Test fun activity200() = compose.shoot("activity-200", fontScale = 2f) { ActivityScreen(populated, "", ActivityActions()) }

    @Test fun activityZero() = compose.shoot("activity-zero") { ActivityScreen(zero, "", ActivityActions()) }

    @Test fun activitySearchEmpty() = compose.shoot("activity-search-empty") {
        ActivityScreen(searchEmpty, "ikea", ActivityActions())
    }

    private val incomeRows = rows.filter { it.type == TxType.INCOME }
    private val incomeLens = populated.copy(
        filter = ActivityFilter.INCOME,
        days = groupByDay(incomeRows),
        summary = summarize(incomeRows),
    )

    @Test fun activityIncomeLens() = compose.shoot("activity-income") { ActivityScreen(incomeLens, "", ActivityActions()) }
}
