package com.tally.core

import org.junit.Assert.assertEquals
import org.junit.Test

class PaceTest {

    @Test fun `even spend is on pace`() {
        val r = PaceReading(budget = 310_00, spent = 100_00, totalDays = 31, elapsedDays = 10)
        assertEquals(100_00, r.expected)
        assertEquals(PaceStatus.ON_PACE, r.status)
        assertEquals(22, r.daysLeft)
        assertEquals(210_00 / 22, r.dailyAllowance)
    }

    @Test fun `spending faster is over pace by the difference`() {
        val r = PaceReading(budget = 310_00, spent = 150_00, totalDays = 31, elapsedDays = 10)
        assertEquals(PaceStatus.OVER_PACE, r.status)
        assertEquals(50_00, r.paceDelta)
    }

    @Test fun `spending slower is under pace`() {
        val r = PaceReading(budget = 310_00, spent = 40_00, totalDays = 31, elapsedDays = 10)
        assertEquals(PaceStatus.UNDER_PACE, r.status)
    }

    @Test fun `past the budget is over budget whatever the day`() {
        val r = PaceReading(budget = 100_00, spent = 120_00, totalDays = 30, elapsedDays = 29)
        assertEquals(PaceStatus.OVER_BUDGET, r.status)
        assertEquals(0, r.dailyAllowance)
    }

    @Test fun `half a day's allowance is still on pace`() {
        // 300/30 = 10 a day; tolerance 5.
        val r = PaceReading(budget = 300_00, spent = 104_00, totalDays = 30, elapsedDays = 10)
        assertEquals(PaceStatus.ON_PACE, r.status)
    }

    @Test fun `no budget never claims a pace`() {
        val r = PaceReading(budget = 0, spent = 50_00, totalDays = 30, elapsedDays = 10)
        assertEquals(PaceStatus.NO_BUDGET, r.status)
        assertEquals(0f, r.spentFraction)
    }

    @Test fun `the tick sits at the share of the month gone`() {
        val r = PaceReading(budget = 100, spent = 0, totalDays = 30, elapsedDays = 15)
        assertEquals(0.5f, r.paceFraction)
    }
}
