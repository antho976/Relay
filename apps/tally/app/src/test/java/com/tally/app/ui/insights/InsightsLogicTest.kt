package com.tally.app.ui.insights

import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.CategoryTotal
import com.tally.app.data.db.DayTotal
import com.tally.app.data.db.TransactionRow
import com.tally.app.ui.common.Dates
import com.tally.core.BudgetPeriod
import com.tally.core.CategoryKind
import com.tally.core.PaceReading
import com.tally.core.TxType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.DayOfWeek
import java.time.LocalDate

class InsightsLogicTest {

    private val period = BudgetPeriod.containing(LocalDate.of(2026, 10, 14))

    private fun day(d: Int): LocalDate = LocalDate.of(2026, 10, d)

    private fun cat(id: Long, name: String = "C$id", kind: CategoryKind = CategoryKind.EXPENSE) =
        CategoryEntity(id = id, name = name, kind = kind, color = (id % 12).toInt(), icon = "cart")

    private fun row(id: Long, amount: Long, date: LocalDate, type: TxType = TxType.EXPENSE) = TransactionRow(
        id = id,
        type = type,
        amount = amount,
        date = date,
        note = "",
        accountId = 1,
        accountName = "Visa",
        toAccountId = null,
        toAccountName = null,
        categoryId = null,
        categoryName = null,
        categoryColor = null,
        categoryIcon = null,
        recurringId = null,
    )

    private fun point(offset: Int, spent: Long, income: Long) =
        TrendPoint(period.shift(offset.toLong()), offset, spent, income)

    // ── Series ──────────────────────────────────────────────────────────────

    @Test fun dailySeriesHasOneSlotPerDayAndSkipsOutsiders() {
        val series = dailySeries(
            listOf(DayTotal(day(2), 300), DayTotal(LocalDate.of(2026, 9, 30), 999), DayTotal(day(31), 50)),
            period,
        )
        assertEquals(31, series.size)
        assertEquals(0L, series[0])
        assertEquals(300L, series[1])
        assertEquals(50L, series[30])
        assertEquals(350L, series.sum())
    }

    @Test fun cumulativeRunsToTodayAndStops() {
        val totals = listOf(DayTotal(day(1), 1_000), DayTotal(day(3), 500), DayTotal(day(20), 9_999))
        assertEquals(listOf(1_000L, 1_000L, 1_500L, 1_500L), cumulative(totals, period, day(4)))
    }

    @Test fun cumulativeOfAPastPeriodCoversEveryDay() {
        val c = cumulative(listOf(DayTotal(day(31), 700)), period, LocalDate.of(2026, 11, 5))
        assertEquals(31, c.size)
        assertEquals(0L, c[29])
        assertEquals(700L, c.last())
    }

    @Test fun cumulativeBeforeThePeriodIsEmpty() {
        assertTrue(cumulative(listOf(DayTotal(day(1), 5)), period, LocalDate.of(2026, 9, 30)).isEmpty())
    }

    @Test fun cumulativeWithNoSpendIsAFlatZeroLine() {
        assertEquals(List(14) { 0L }, cumulative(emptyList(), period, day(14)))
    }

    // ── Against last month ──────────────────────────────────────────────────

    @Test fun theCurrentPeriodReadsLastMonthByTheSameDay() {
        // Day 14 of October: September up to and including the 14th.
        assertEquals(LocalDate.of(2026, 9, 15), previousPeriodEnd(period, 0, day(14)))
    }

    @Test fun lateInALongMonthTheShortOneBeforeCountsWhole() {
        // Day 31 of October against a 30-day September: all of September, never into October.
        assertEquals(LocalDate.of(2026, 10, 1), previousPeriodEnd(period, 0, day(31)))
    }

    @Test fun aPastPeriodReadsTheWholeOneBefore() {
        val september = period.shift(-1)
        // August has 31 days; a same-day cut at September's 30 would drop the 31st.
        assertEquals(LocalDate.of(2026, 9, 1), previousPeriodEnd(september, -1, day(14)))
    }

    // ── Pace and pointer ────────────────────────────────────────────────────

    @Test fun paceAtMatchesThePaceReading() {
        for (d in 0 until 31) {
            assertEquals(PaceReading(290_000, 0, 31, d + 1).expected, paceAt(290_000, d, 31))
        }
    }

    @Test fun paceAtWithoutABudgetIsZero() {
        assertEquals(0L, paceAt(0, 10, 31))
        assertEquals(0L, paceAt(-5, 10, 31))
    }

    @Test fun dayIndexAtMapsAcrossAndClampsToDrawnDays() {
        assertEquals(0, dayIndexAt(0f, 300, 31, 31, rtl = false))
        assertEquals(15, dayIndexAt(150f, 300, 31, 31, rtl = false))
        assertEquals(30, dayIndexAt(300f, 300, 31, 31, rtl = false))
        // Days still ahead are not drawn, so a finger past today reads today.
        assertEquals(13, dayIndexAt(300f, 300, 31, 14, rtl = false))
        assertEquals(0, dayIndexAt(-20f, 300, 31, 14, rtl = false))
    }

    @Test fun dayIndexAtMirrorsInRtl() {
        assertEquals(30, dayIndexAt(0f, 300, 31, 31, rtl = true))
        assertEquals(0, dayIndexAt(300f, 300, 31, 31, rtl = true))
    }

    // ── Heat ────────────────────────────────────────────────────────────────

    @Test fun heatStepIsUnlitWithoutSpend() {
        assertEquals(0, heatStep(0, 10_000))
        assertEquals(0, heatStep(-5, 10_000))
        assertEquals(0, heatStep(100, 0))
    }

    @Test fun heatStepClimbsByQuarter() {
        assertEquals(1, heatStep(1, 10_000))
        assertEquals(1, heatStep(2_500, 10_000))
        assertEquals(2, heatStep(2_501, 10_000))
        assertEquals(2, heatStep(5_000, 10_000))
        assertEquals(3, heatStep(7_500, 10_000))
        assertEquals(4, heatStep(7_501, 10_000))
        assertEquals(4, heatStep(10_000, 10_000))
    }

    // ── Breakdown ───────────────────────────────────────────────────────────

    @Test fun slicesRankLargestFirstAndNameUncategorized() {
        val cats = (1L..3L).associateWith { cat(it) }
        val slices = rankSlices(
            listOf(CategoryTotal(1, 500, 2), CategoryTotal(null, 900, 1), CategoryTotal(2, 1_500, 3), CategoryTotal(3, 0, 0)),
            cats,
        )
        assertEquals(listOf("C2", "Uncategorized", "C1"), slices.map { it.name })
        val none = slices[1]
        assertTrue(none.isUncategorized)
        assertNull(none.categoryId)
        assertNull(none.color)
        assertEquals(listOf(2L, null, 1L), slices.map { it.categoryId })
    }

    @Test fun aCategoryThatNoLongerResolvesPoolsWithUncategorized() {
        val slices = rankSlices(listOf(CategoryTotal(99, 200, 1), CategoryTotal(null, 300, 2)), emptyMap())
        assertEquals(1, slices.size)
        assertEquals(500L, slices[0].total)
        assertEquals(3, slices[0].count)
    }

    @Test fun eightCategoriesShowWithoutAFold() {
        val cats = (1L..8L).associateWith { cat(it) }
        val slices = rankSlices((1L..8L).map { CategoryTotal(it, it * 100, 1) }, cats)
        assertEquals(8, slices.size)
        assertFalse(slices.any { it.isFolded })
    }

    @Test fun pastEightTheTailFoldsIntoEverythingElse() {
        val cats = (1L..10L).associateWith { cat(it) }
        val slices = rankSlices((1L..10L).map { CategoryTotal(it, it * 100, 2) }, cats)
        assertEquals(8, slices.size)
        assertEquals((10L downTo 4L).toList(), slices.take(7).map { it.categoryId })
        val rest = slices.last()
        assertTrue(rest.isFolded)
        assertFalse(rest.isUncategorized)
        assertEquals("Everything else", rest.name)
        assertEquals(3, rest.folded)
        assertEquals(600L, rest.total)
        assertEquals(6, rest.count)
    }

    // ── Trend ───────────────────────────────────────────────────────────────

    @Test fun trendAveragesLeaveOutTheRunningPeriodAndEmptyOnes() {
        val points = listOf(
            point(-5, 0, 0),
            point(-4, 200_000, 400_000),
            point(-3, 300_000, 400_000),
            point(-2, 250_000, 430_000),
            point(-1, 0, 0),
            point(0, 50_000, 0),
        )
        val t = summarizeTrend(points, budget = 280_000)
        assertEquals(3, t.months)
        assertEquals(250_000L, t.averageSpent)
        assertEquals(410_000L, t.averageIncome)
        assertEquals(160_000L, t.averageNet)
        assertEquals(2, t.highest)
        assertEquals(1, t.lowest)
        assertEquals(1, t.overBudget)
        assertFalse(t.partial)
    }

    @Test fun trendFallsBackToTheRunningPeriodAlone() {
        val points = (-5..-1).map { point(it, 0, 0) } + point(0, 50_000, 0)
        val t = summarizeTrend(points, budget = null)
        assertEquals(1, t.months)
        assertEquals(50_000L, t.averageSpent)
        assertTrue(t.partial)
        assertEquals(5, t.highest)
        assertNull(t.lowest)
        assertEquals(0, t.overBudget)
    }

    @Test fun trendWithNothingReadsZero() {
        val t = summarizeTrend((-5..0).map { point(it, 0, 0) }, budget = 100_000)
        assertEquals(TrendSummary(), t)
    }

    // ── Calendar ────────────────────────────────────────────────────────────

    @Test fun calendarReadsTheDaysSoFar() {
        // Oct 1 2026 is a Thursday; the 3rd is a Saturday.
        val daily = listOf(1_000L, 0L, 200L, 0L, 500L, 0L) + List(25) { 0L }
        val c = summarizeCalendar(daily, period, elapsed = 6)
        assertEquals(6, c.daysSoFar)
        assertEquals(3, c.activeDays)
        assertEquals(3, c.quietDays)
        assertEquals(1, c.longestQuietRun)
        assertEquals(1_700L, c.total)
        assertEquals(566L, c.perActiveDay)
        assertEquals(day(1), c.busiestDate)
        assertEquals(1_000L, c.busiestTotal)
        assertEquals(DayOfWeek.THURSDAY, c.topWeekday)
        assertEquals(1, c.topWeekdayDays)
        assertEquals(200L, c.weekendTotal)
    }

    @Test fun calendarWeekdaysFollowTheWeekStart() {
        // Oct 1 to 14 2026: two of every weekday.
        val daily = List(14) { if (it == 5) 3_000L else 100L } + List(17) { 0L }
        val monday = summarizeCalendar(daily, period, elapsed = 14, weekStartsMonday = true)
        assertEquals(7, monday.weekdays.size)
        assertEquals(DayOfWeek.MONDAY, monday.weekdays.first().day)
        assertEquals(DayOfWeek.SUNDAY, monday.weekdays.last().day)
        assertTrue(monday.weekdays.all { it.days == 2 })
        // The 6th is a Tuesday: 3,000 and the 13th's 100.
        val tuesday = monday.weekdays[1]
        assertEquals(3_100L, tuesday.total)
        assertEquals(1_550L, tuesday.average)
        assertEquals(DayOfWeek.TUESDAY, monday.topWeekday)
        val sunday = summarizeCalendar(daily, period, elapsed = 14, weekStartsMonday = false)
        assertEquals(DayOfWeek.SUNDAY, sunday.weekdays.first().day)
        assertEquals(DayOfWeek.SATURDAY, sunday.weekdays.last().day)
    }

    @Test fun busiestWeekdayIsByTheAverageDay() {
        // Thursday the 1st and 8th at 300 each (600 over two); Tuesday the 6th alone at 500.
        val daily = List(31) { i ->
            when (i) {
                0, 7 -> 300L
                5 -> 500L
                else -> 0L
            }
        }
        val c = summarizeCalendar(daily, period, elapsed = 8)
        assertEquals(DayOfWeek.TUESDAY, c.topWeekday)
        assertEquals(500L, c.topWeekdayTotal)
        assertEquals(1, c.topWeekdayDays)
    }

    @Test fun calendarCountsTheLongestQuietRun() {
        val daily = listOf(100L, 0L, 0L, 0L, 50L, 0L, 0L) + List(24) { 0L }
        assertEquals(3, summarizeCalendar(daily, period, elapsed = 7).longestQuietRun)
    }

    @Test fun calendarWithNoSpendHasNoBusiestDay() {
        val c = summarizeCalendar(List(31) { 0L }, period, elapsed = 14)
        assertEquals(14, c.quietDays)
        assertEquals(0, c.activeDays)
        assertNull(c.busiestDate)
        assertNull(c.topWeekday)
    }

    // ── Assembly ────────────────────────────────────────────────────────────

    @Test fun buildInsightsAssemblesThePeriod() {
        val today = day(14)
        val rows = listOf(
            row(1, 9_000, day(14)),
            row(2, 400, day(14)),
            row(3, 15_000, day(10)),
            row(4, 200_000, day(2), type = TxType.INCOME),
            row(5, 2_000, day(3)),
        )
        val s = buildInsights(
            today = today,
            period = period,
            offset = 0,
            weekStartsMonday = true,
            dayTotals = listOf(DayTotal(day(3), 2_000), DayTotal(day(10), 15_000), DayTotal(day(14), 9_400), DayTotal(day(20), 1_000)),
            expenseByCategory = listOf(CategoryTotal(1, 26_400, 4), CategoryTotal(null, 1_000, 1)),
            incomeByCategory = listOf(CategoryTotal(5, 200_000, 1)),
            rows = rows,
            categories = listOf(cat(1, "Groceries"), cat(5, "Salary", CategoryKind.INCOME)),
            budget = 290_000,
            lastPeriodSameDay = 20_000,
            trend = (-5..0).map { point(it, 10_000, 0) },
        )
        assertEquals(14, s.elapsedDays)
        assertEquals(14, s.cumulative.size)
        assertEquals(26_400L, s.cumulative.last())
        // The future-dated entry counts toward the period, not the line drawn to today.
        assertEquals(27_400L, s.spent)
        assertEquals(200_000L, s.income)
        assertEquals(5, s.expenseCount)
        assertEquals(listOf(3L, 1L, 5L), s.largest.map { it.id })
        assertEquals(26_000L, s.largestTotal)
        assertEquals(4, s.heat[9])
        assertEquals(3, s.heat[13])
        assertEquals(1, s.heat[2])
        assertEquals(0, s.heat[19])
        assertEquals(0, s.heat[0])
        val reading = s.reading
        assertNotNull(reading)
        assertEquals(290_000L, reading!!.budget)
        assertEquals(2, s.categoryCount)
        assertEquals(listOf("Groceries", "Uncategorized"), s.categories.map { it.name })
        assertEquals(listOf("Salary"), s.incomeSources.map { it.name })
        assertEquals(2, s.entriesByDay[day(14)]?.size)
        assertEquals(6, s.trend.size)
        assertTrue(s.isCurrentPeriod)
        assertTrue(s.loaded)
    }

    @Test fun aZeroBudgetReadsAsNoBudget() {
        val s = buildInsights(
            today = day(14), period = period, offset = 0, weekStartsMonday = false,
            dayTotals = emptyList(), expenseByCategory = emptyList(), incomeByCategory = emptyList(),
            rows = emptyList(), categories = emptyList(), budget = 0, lastPeriodSameDay = 0, trend = emptyList(),
        )
        assertNull(s.budget)
        assertNull(s.reading)
        assertNull(s.projected)
        assertEquals(List(14) { 0L }, s.cumulative)
    }

    @Test fun aPastPeriodReadsEveryDay() {
        val past = period.shift(-1)
        val s = buildInsights(
            today = day(14), period = past, offset = -1, weekStartsMonday = true,
            dayTotals = listOf(DayTotal(past.lastDay, 3_000)), expenseByCategory = emptyList(), incomeByCategory = emptyList(),
            rows = emptyList(), categories = emptyList(), budget = null, lastPeriodSameDay = 0, trend = emptyList(),
        )
        assertFalse(s.isCurrentPeriod)
        assertEquals(past.days, s.elapsedDays)
        assertEquals(past.days, s.cumulative.size)
        assertEquals(4, s.heat.last())
        assertNull(s.projected)
        assertEquals(0, s.daysLeft)
    }

    // ── Day sheet copy ──────────────────────────────────────────────────────

    @Test fun anEmptyDaySheetReadsRelativeDaysAsWords() {
        assertEquals("Nothing logged today", nothingLoggedLine(day(14), day(14)))
        // Never "Nothing logged on Yesterday".
        assertEquals("Nothing logged yesterday", nothingLoggedLine(day(13), day(14)))
    }

    @Test fun anEmptyDaySheetNamesAnyOtherDayByItsDate() {
        assertEquals("Nothing logged on " + Dates.day(day(6), day(14)), nothingLoggedLine(day(6), day(14)))
        assertFalse(nothingLoggedLine(day(6), day(14)).contains("2026-"))
    }
}
