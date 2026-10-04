package com.margin.core

import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.LocalDate

class RecurrenceTest {

    private fun d(m: Int, day: Int, y: Int = 2026) = LocalDate.of(y, m, day)

    @Test fun `month-end anchors do not drift`() {
        val r = Recurrence(d(1, 31), Frequency.MONTHLY)
        assertEquals(d(2, 28), r.occurrence(1))
        assertEquals(d(3, 31), r.occurrence(2))
        assertEquals(d(3, 31), r.after(d(2, 28)))
    }

    @Test fun `on or after returns the anchor before it starts`() {
        val r = Recurrence(d(5, 10), Frequency.MONTHLY)
        assertEquals(d(5, 10), r.onOrAfter(d(1, 1)))
    }

    @Test fun `biweekly salary`() {
        val r = Recurrence(d(10, 2), Frequency.WEEKLY, 2)
        assertEquals(d(10, 16), r.after(d(10, 2)))
        assertEquals(listOf(d(10, 2), d(10, 16), d(10, 30)), r.between(d(10, 1), d(10, 31)))
    }

    @Test fun `yearly on a leap day lands on Feb 28 then back`() {
        val r = Recurrence(LocalDate.of(2028, 2, 29), Frequency.YEARLY)
        assertEquals(LocalDate.of(2029, 2, 28), r.occurrence(1))
        assertEquals(LocalDate.of(2032, 2, 29), r.occurrence(4))
    }

    @Test fun `between is capped`() {
        val r = Recurrence(d(1, 1, 2000), Frequency.WEEKLY)
        assertEquals(10, r.between(d(1, 1, 2000), d(1, 1, 2026), limit = 10).size)
    }

    @Test fun `per month factor`() {
        assertEquals(1.0, Recurrence(d(1, 1), Frequency.MONTHLY).perMonthFactor(), 1e-9)
        assertEquals(1.0 / 12, Recurrence(d(1, 1), Frequency.YEARLY).perMonthFactor(), 1e-9)
    }
}
