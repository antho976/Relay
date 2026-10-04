package com.tally.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test
import java.util.Locale

class CopyTest {

    private val fmt = MoneyFormatter("CAD", Locale.CANADA)

    @Test fun `margin line names the daily allowance`() {
        val r = PaceReading(budget = 310_00, spent = 100_00, totalDays = 31, elapsedDays = 10)
        assertEquals("$10 a day for 22 days", Copy.marginLine(r, fmt))
    }

    @Test fun `over budget names the overrun`() {
        val r = PaceReading(budget = 100_00, spent = 130_00, totalDays = 30, elapsedDays = 20)
        assertEquals("$30 over budget, 11 days left", Copy.marginLine(r, fmt))
    }

    @Test fun `last day reads as today`() {
        val r = PaceReading(budget = 100_00, spent = 90_00, totalDays = 30, elapsedDays = 30)
        assertEquals("$10 left for today", Copy.marginLine(r, fmt))
    }

    @Test fun `plural is right at one`() {
        assertEquals("1 day", Copy.plural(1, "day"))
        assertEquals("2 days", Copy.plural(2, "day"))
    }

    @Test fun `goal line divides what is left over the months`() {
        assertEquals("$100 a month for 3 months", Copy.goalLine(700_00, 1_000_00, 3, fmt))
        assertEquals("Reached", Copy.goalLine(1_000_00, 1_000_00, 3, fmt))
    }

    @Test fun `nothing by this day last month never claims a first month`() {
        assertEquals("Nothing by this day last month", Copy.versusLastLine(4_00, 0, fmt))
        assertEquals("Nothing spent yet", Copy.versusLastLine(0, 0, fmt))
        assertEquals("$6 more than last month by this day", Copy.versusLastLine(10_00, 4_00, fmt))
        assertEquals("Level with last month by this day", Copy.versusLastLine(4_00, 4_00, fmt))
    }

    @Test fun `no generated line breaks the voice rules`() {
        val readings = listOf(
            PaceReading(310_00, 100_00, 31, 10), PaceReading(310_00, 200_00, 31, 10),
            PaceReading(310_00, 10_00, 31, 10), PaceReading(100_00, 300_00, 31, 31),
            PaceReading(0, 10_00, 31, 10),
        )
        val lines = readings.flatMap { listOf(Copy.marginLine(it, fmt), Copy.paceLine(it, fmt)) } +
            listOf(Copy.dueLine(-3), Copy.dueLine(0), Copy.dueLine(1), Copy.dueLine(9)) +
            listOf(Copy.versusLastLine(10, 0, fmt), Copy.versusLastLine(10, 20, fmt), Copy.versusLastLine(30, 20, fmt))
        lines.forEach { line -> Copy.banned.forEach { bad -> assertFalse("\"$line\" contains \"$bad\"", line.contains(bad, ignoreCase = true)) } }
    }
}
