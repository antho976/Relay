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

    // ── Spending against the limit ───────────────────────────────────────────

    private val fmt = com.tally.core.MoneyFormatter("CAD", Locale.CANADA)

    @Test fun theHeroSpeaksOfTheLimitOnceItIsReached() {
        val under = PaceReading(300_000, 141_220, 31, 13)
        assertEquals(com.tally.core.Copy.daysLine(under), heroLine(under, fmt).text)
        val over = PaceReading(300_000, 312_000, 31, 13)
        assertEquals(com.tally.app.ui.common.ToneLine("$120 past your $3,000 limit", com.tally.app.ui.common.LineTone.HELD), heroLine(over, fmt))
        assertEquals("All of your $3,000 is spent", heroLine(PaceReading(300_000, 300_000, 31, 13), fmt).text)
        assertEquals(false, heroLine(over, fmt).text.contains("-"))
    }

    @Test fun todayReadsAgainstItsShareOfTheRoom() {
        // $3,000 for 31 days, $1,000 spent before today, day 13: $2,000 over 19 days is $105 for today.
        val r = PaceReading(300_000, 100_000 + 2_300, 31, 13)
        assertEquals("$82 left for today", todayLine(2_300, r, fmt).text)
        assertEquals("$105 for today", todayLine(0, PaceReading(300_000, 100_000, 31, 13), fmt).text)
        val big = PaceReading(300_000, 100_000 + 15_000, 31, 13)
        assertEquals(com.tally.app.ui.common.ToneLine("$45 past today's share", com.tally.app.ui.common.LineTone.WARN), todayLine(15_000, big, fmt))
        // Past the limit the hero carries the red; today only says what it took.
        assertEquals(com.tally.app.ui.common.ToneLine("Spent so far today"), todayLine(4_000, PaceReading(300_000, 310_000, 31, 13), fmt))
        assertEquals("Nothing spent yet today", todayLine(0, null, fmt).text)
    }

    @Test fun theStatusBarNeverShowsANegative() {
        assertEquals("$1,588 left · 18 days", statusMonthLine(158_800, 141_200, 18, fmt).text)
        val over = statusMonthLine(-12_000, 312_000, 18, fmt)
        assertEquals("Limit reached · 18 days", over.text)
        assertEquals(com.tally.app.ui.common.LineTone.HELD, over.tone)
        assertEquals("Limit reached · 1 day", statusMonthLine(0, 300_000, 1, fmt).text)
        assertEquals("$1,412 spent · 18 days", statusMonthLine(null, 141_200, 18, fmt).text)
    }

    @Test fun whereItWentIsTheLargestFewNamed() {
        val cats = listOf(
            com.tally.app.data.db.CategoryEntity(id = 1, name = "Groceries", kind = com.tally.core.CategoryKind.EXPENSE, color = 0, icon = "cart"),
            com.tally.app.data.db.CategoryEntity(id = 2, name = "Dining", kind = com.tally.core.CategoryKind.EXPENSE, color = 6, icon = "dining"),
        )
        val totals = listOf(
            com.tally.app.data.db.CategoryTotal(2, 23_100, 4),
            com.tally.app.data.db.CategoryTotal(1, 41_800, 9),
            com.tally.app.data.db.CategoryTotal(null, 1_200, 1),
            com.tally.app.data.db.CategoryTotal(3, 0, 0),
        )
        val top = topCategories(totals, cats)
        assertEquals(listOf("Groceries", "Dining", "Uncategorized"), top.map { it.name })
        assertEquals(listOf(41_800L, 23_100L, 1_200L), top.map { it.total })
        assertEquals(2, topCategories(totals, cats, limit = 2).size)
    }
}
