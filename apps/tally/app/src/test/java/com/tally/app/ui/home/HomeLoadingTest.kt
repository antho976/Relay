package com.tally.app.ui.home

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.testing.Fixtures
import com.tally.app.ui.common.Dates
import com.tally.app.ui.theme.TallyTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Home must not draw its zero state over a ledger it has not read yet. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class HomeLoadingTest {

    @get:Rule val compose = createComposeRule()

    /** What the view model holds before Room has answered: every figure a placeholder zero. */
    private val unread = HomeState(today = Fixtures.TODAY, period = Fixtures.PERIOD)

    @Test fun beforeTheFirstReadOnlyTheFrameShows() {
        compose.setContent { TallyTheme { HomeScreen(unread, HomeActions()) } }
        compose.onNodeWithText(Dates.period(Fixtures.PERIOD, Fixtures.TODAY)).assertExists()
        compose.onNodeWithText("LEFT FROM INCOME").assertDoesNotExist()
    }

    @Test fun onceReadAnEmptyMonthShowsItsZeroState() {
        compose.setContent { TallyTheme { HomeScreen(unread.copy(loaded = true), HomeActions()) } }
        compose.onNodeWithText("LEFT FROM INCOME").assertExists()
    }
}
