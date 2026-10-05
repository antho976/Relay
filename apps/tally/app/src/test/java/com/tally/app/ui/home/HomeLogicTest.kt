package com.tally.app.ui.home

import androidx.compose.ui.unit.dp
import com.tally.core.BudgetPeriod
import com.tally.core.PaceReading
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import java.time.LocalDate
import java.util.Locale

class HomeLogicTest {

    private lateinit var saved: Locale

    @Before fun english() {
        saved = Locale.getDefault()
        Locale.setDefault(Locale.CANADA)
    }

    @After fun restore() = Locale.setDefault(saved)

    @Test fun oneAccountReadsSingular() {
        assertEquals("NET ACROSS 1 ACCOUNT", netAcrossLabel(1))
        assertEquals("NET ACROSS 4 ACCOUNTS", netAcrossLabel(4))
    }

    @Test fun weekDescriptionNamesEachDaySoFarInFull() {
        // Sunday 11 Oct 2026 starts the week; today is the Wednesday, so four days are read.
        val week = listOf(4_200L, 9_800L, 2_100L, 3_400L, 0L, 0L, 0L)
        val text = weekDescription(LocalDate.of(2026, 10, 11), week, todayInWeek = 3) { "$" + it / 100 }
        assertEquals("This week: Sunday $42, Monday $98, Tuesday $21, Wednesday $34", text)
    }

    @Test fun weekDescriptionTellsSaturdayFromSundayAndTuesdayFromThursday() {
        val text = weekDescription(LocalDate.of(2026, 10, 11), List(7) { 0L }, todayInWeek = 6) { "$" + it }
        assertEquals(
            "This week: Sunday $0, Monday $0, Tuesday $0, Wednesday $0, Thursday $0, Friday $0, Saturday $0",
            text,
        )
    }

    @Test fun weekDescriptionSaysWhatTheDaysBeforeThePeriodSpent() {
        // Monday 28 Sep to Sunday 4 Oct; October's period starts on the Thursday.
        val week = listOf(5_000L, 7_000L, 7_700L, 3_000L, 4_000L, 0L, 2_000L)
        val text = weekDescription(
            LocalDate.of(2026, 9, 28), week, todayInWeek = 6,
            spentBeforePeriod = 19_700L, periodStart = LocalDate.of(2026, 10, 1),
        ) { "$" + it / 100 }
        assertEquals(
            "This week: Monday $50, Tuesday $70, Wednesday $77, Thursday $30, Friday $40, Saturday $0, Sunday $20, " +
                "$197 of it before 1 October",
            text,
        )
    }

    @Test fun aWeekThatOpensInLastMonthCountsWhatThoseDaysSpent() {
        // Today is Sunday 4 Oct with the week from Monday: 28, 29 and 30 Sep belong to September.
        val today = LocalDate.of(2026, 10, 4)
        val state = HomeState(
            today = today,
            period = BudgetPeriod.containing(today),
            week = listOf(5_000L, 7_000L, 7_700L, 3_000L, 4_000L, 0L, 2_000L),
            weekStart = LocalDate.of(2026, 9, 28),
        )
        assertEquals(3, state.weekDaysBeforePeriod)
        assertEquals(19_700L, state.weekSpentBeforePeriod)
        assertEquals(28_700L, state.weekSpent)
    }

    @Test fun aWeekInsideThePeriodHasNothingBeforeIt() {
        val today = LocalDate.of(2026, 10, 14)
        val state = HomeState(
            today = today,
            period = BudgetPeriod.containing(today),
            week = listOf(4_200L, 9_800L, 2_100L, 3_400L, 0L, 0L, 0L),
            weekStart = LocalDate.of(2026, 10, 11),
        )
        assertEquals(0, state.weekDaysBeforePeriod)
        assertEquals(0L, state.weekSpentBeforePeriod)
    }

    @Test fun aPaydayPeriodCountsOnlyTheDaysBeforeItsStartDay() {
        // Paid on the 15th: the period starts Thursday 15 Oct, the week on Sunday 11 Oct.
        val today = LocalDate.of(2026, 10, 15)
        val state = HomeState(
            today = today,
            period = BudgetPeriod.containing(today, startDay = 15),
            week = listOf(1_000L, 2_000L, 3_000L, 4_000L, 500L, 0L, 0L),
            weekStart = LocalDate.of(2026, 10, 11),
        )
        assertEquals(4, state.weekDaysBeforePeriod)
        assertEquals(10_000L, state.weekSpentBeforePeriod)
        assertEquals(10_500L, state.weekSpent)
    }

    @Test fun envelopesLeadWithTheFurthestOverPaceForTheirSize() {
        // Day 14 of October's 31.
        val period = BudgetPeriod.containing(LocalDate.of(2026, 10, 14))
        fun envelope(id: Long, name: String, budget: Long, spent: Long) =
            Envelope(id, name, null, null, PaceReading(budget, spent, period.days, 14))
        val dining = envelope(2, "Dining", 32_000, 21_800)
        val groceries = envelope(1, "Groceries", 50_000, 23_100)
        val transport = envelope(3, "Transport", 12_000, 4_100)
        val shopping = envelope(4, "Shopping", 15_000, 19_999)
        // Further over pace in dollars than Shopping, but by less of its own size.
        val rent = envelope(5, "Rent", 200_000, 105_000)
        assertEquals(
            listOf("Shopping", "Dining", "Rent", "Groceries", "Transport"),
            sortEnvelopes(listOf(dining, groceries, transport, rent, shopping)).map { it.name },
        )
    }

    @Test fun homeMatchesTheWidthItIsGiven() {
        assertEquals(HomeLayout.PHONE, homeLayout(411.dp))
        assertEquals(HomeLayout.PHONE, homeLayout(599.dp))
        assertEquals(HomeLayout.COLUMN, homeLayout(600.dp))
        assertEquals(HomeLayout.COLUMN, homeLayout(839.dp))
        assertEquals(HomeLayout.PANES, homeLayout(840.dp))
        // A tablet in landscape: 1280dp less the 88dp rail.
        assertEquals(HomeLayout.PANES, homeLayout(1192.dp))
    }
}
