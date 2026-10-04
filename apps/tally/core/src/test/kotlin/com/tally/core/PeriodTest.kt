package com.tally.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

class PeriodTest {

    @Test fun `calendar month by default`() {
        val p = BudgetPeriod.containing(LocalDate.of(2026, 10, 4))
        assertEquals(LocalDate.of(2026, 10, 1), p.start)
        assertEquals(LocalDate.of(2026, 11, 1), p.endExclusive)
        assertEquals(31, p.days)
    }

    @Test fun `a start day before today stays in this month`() {
        val p = BudgetPeriod.containing(LocalDate.of(2026, 10, 20), startDay = 15)
        assertEquals(LocalDate.of(2026, 10, 15), p.start)
        assertEquals(LocalDate.of(2026, 11, 15), p.endExclusive)
    }

    @Test fun `a start day after today belongs to last month's cycle`() {
        val p = BudgetPeriod.containing(LocalDate.of(2026, 10, 4), startDay = 15)
        assertEquals(LocalDate.of(2026, 9, 15), p.start)
        assertEquals(LocalDate.of(2026, 10, 15), p.endExclusive)
    }

    @Test fun `start days clamp to 28 so February has one`() {
        val p = BudgetPeriod.containing(LocalDate.of(2027, 2, 28), startDay = 31)
        assertEquals(LocalDate.of(2027, 2, 28), p.start)
    }

    @Test fun `elapsed and left both count today`() {
        val p = BudgetPeriod.containing(LocalDate.of(2026, 10, 1))
        val first = LocalDate.of(2026, 10, 1)
        val last = LocalDate.of(2026, 10, 31)
        assertEquals(1, p.elapsedDays(first)); assertEquals(31, p.daysLeft(first))
        assertEquals(31, p.elapsedDays(last)); assertEquals(1, p.daysLeft(last))
        assertEquals(0, p.daysLeft(LocalDate.of(2026, 11, 3)))
        assertEquals(31, p.daysLeft(LocalDate.of(2026, 9, 3)))
    }

    @Test fun `contains is half open`() {
        val p = BudgetPeriod.containing(LocalDate.of(2026, 10, 1))
        assertTrue(LocalDate.of(2026, 10, 31) in p)
        assertFalse(LocalDate.of(2026, 11, 1) in p)
    }

    @Test fun `shift walks whole cycles`() {
        val p = BudgetPeriod.containing(LocalDate.of(2026, 1, 20), startDay = 15)
        assertEquals(LocalDate.of(2025, 12, 15), p.shift(-1).start)
        assertEquals(LocalDate.of(2026, 2, 15), p.shift(1).start)
    }
}
