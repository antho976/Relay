package com.forge.app.ui.profile

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.forge.app.ui.theme.ForgeTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.time.LocalDate
import java.time.YearMonth
import java.time.format.DateTimeFormatter
import java.util.Locale

/**
 * The ACTIVITY calendar's two interactions: tapping a day, and paging months.
 *
 * Deliberately NOT a screenshot golden. The grid is drawn from `LocalDate.now()` — its month name,
 * its number of week rows and which cells count as future all change on their own — so a golden
 * would go red on a calendar boundary rather than on a regression, which is the fastest way to
 * teach everyone to re-record without looking. These assertions hold in any month.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class ProfileActivityMonthTest {

    @get:Rule
    val compose = createComposeRule()

    private val today: LocalDate = LocalDate.now()

    /**
     * A day this month with something on it, and one last month, picked so both are safely inside
     * their month whatever today's date is. Two months of history is what makes "page back" and
     * "stop at the oldest month" both testable.
     */
    private val litThisMonth: LocalDate = YearMonth.from(today).atDay(1)
    private val litLastMonth: LocalDate = YearMonth.from(today).minusMonths(1).atDay(1)

    private val activity = mapOf(
        litThisMonth.toEpochDay() to 2,
        litLastMonth.toEpochDay() to 1
    )

    private fun reading(date: LocalDate, sessions: Int): String {
        val day = date.format(DateTimeFormatter.ofPattern("MMMM d", Locale.getDefault()))
        return if (sessions == 1) "$day, 1 session" else "$day, $sessions sessions"
    }

    private fun monthLabel(month: YearMonth): String =
        "${month.month.getDisplayName(java.time.format.TextStyle.FULL, Locale.getDefault()).uppercase()} ${month.year}"

    private fun setContent(onDayTap: ((LocalDate) -> Unit)? = null) {
        compose.setContent {
            ForgeTheme {
                ProfileActivityMonth(
                    activityByDay = activity,
                    streakDays = 3,
                    longestStreakDays = 9,
                    onBg = MaterialTheme.colorScheme.onBackground,
                    muted = MaterialTheme.colorScheme.onSurfaceVariant,
                    hue = MaterialTheme.colorScheme.primary,
                    onDayTap = onDayTap
                )
            }
        }
    }

    @Test
    fun tappingALitDayOpensThatDay() {
        var tapped: LocalDate? = null
        setContent { tapped = it }
        compose.onNodeWithContentDescription(reading(litThisMonth, 2)).performClick()
        assertEquals(litThisMonth, tapped)
    }

    /** A rest day has nothing to show, so it stays inert rather than opening an empty sheet. */
    @Test
    fun tappingARestDayDoesNothing() {
        val rest = litThisMonth.plusDays(1)
        var tapped: LocalDate? = null
        setContent { tapped = it }
        compose.onNodeWithContentDescription("${rest.format(DateTimeFormatter.ofPattern("MMMM d", Locale.getDefault()))}, rest day")
            .performClick()
        assertNull(tapped)
    }

    @Test
    fun theEarlierChevronPagesBackAMonth() {
        setContent {}
        compose.onNodeWithText(monthLabel(YearMonth.from(today))).assertIsDisplayed()
        compose.onNodeWithContentDescription("Earlier month").performClick()
        compose.onNodeWithText(monthLabel(YearMonth.from(today).minusMonths(1))).assertIsDisplayed()
        // Last month's lit day is now on screen, so the grid really did redraw the month.
        compose.onNodeWithContentDescription(reading(litLastMonth, 1)).assertIsDisplayed()
    }

    /** Forward stops at this month; back stops at the oldest month there is anything in. */
    @Test
    fun theChevronsStopAtBothEnds() {
        setContent {}
        compose.onNodeWithContentDescription("Later month").assertIsNotEnabled()
        compose.onNodeWithContentDescription("Earlier month").assertIsEnabled()

        compose.onNodeWithContentDescription("Earlier month").performClick()
        compose.onNodeWithContentDescription("Later month").assertIsEnabled()
        compose.onNodeWithContentDescription("Earlier month").assertIsNotEnabled()
    }
}
