package com.tally.app.ui.insights

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.testing.Fixtures
import com.tally.app.ui.theme.TallyTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Insights must not read "Nothing spent" over a month it has not read yet. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class InsightsLoadingTest {

    @get:Rule val compose = createComposeRule()

    private val empty = buildInsights(
        today = Fixtures.TODAY,
        period = Fixtures.PERIOD,
        offset = 0,
        weekStartsMonday = true,
        dayTotals = emptyList(),
        expenseByCategory = emptyList(),
        incomeByCategory = emptyList(),
        rows = emptyList(),
        categories = emptyList(),
        budget = null,
        lastPeriodSameDay = 0,
        trend = emptyList(),
    )

    @Test fun beforeTheFirstReadOnlyTheFrameShows() {
        compose.setContent { TallyTheme { InsightsScreen(empty.copy(loaded = false), InsightsLens.MONTH, InsightsActions()) } }
        compose.onNodeWithText("Insights").assertExists()
        compose.onNodeWithText("SPENT SO FAR").assertDoesNotExist()
    }

    @Test fun onceReadAnEmptyMonthShowsItsZeroState() {
        compose.setContent { TallyTheme { InsightsScreen(empty, InsightsLens.MONTH, InsightsActions()) } }
        compose.onNodeWithText("SPENT SO FAR").assertExists()
    }
}
