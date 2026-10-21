package com.forge.app.ui.settings

import com.forge.app.data.db.entities.VacationPeriod
import java.time.LocalDate
import org.junit.Assert.assertEquals
import org.junit.Test

class RootReadingsTest {

    private val today = LocalDate.parse("2026-10-17")

    @Test fun `no holidays reads none planned`() {
        assertEquals("None planned", holidayReading(emptyList(), today))
    }

    @Test fun `a holiday under way names its last day`() {
        val periods = listOf(VacationPeriod(startDate = "2026-10-15", endDate = "2026-10-24"))
        assertEquals("Away until Oct 24", holidayReading(periods, today))
    }

    @Test fun `a holiday ending today is still under way`() {
        val periods = listOf(VacationPeriod(startDate = "2026-10-10", endDate = "2026-10-17"))
        assertEquals("Away until Oct 17", holidayReading(periods, today))
    }

    @Test fun `the nearest upcoming holiday leads when none is under way`() {
        val periods = listOf(
            VacationPeriod(startDate = "2026-12-20", endDate = "2026-12-31"),
            VacationPeriod(startDate = "2026-11-03", endDate = "2026-11-09"),
            VacationPeriod(startDate = "2026-09-01", endDate = "2026-09-07")
        )
        assertEquals("Next from Nov 3", holidayReading(periods, today))
    }

    @Test fun `past holidays and malformed rows read none planned`() {
        val periods = listOf(
            VacationPeriod(startDate = "2026-09-01", endDate = "2026-09-07"),
            VacationPeriod(startDate = "not a date", endDate = "2026-12-01")
        )
        assertEquals("None planned", holidayReading(periods, today))
    }

    @Test fun `cardio count reads as sports of your own`() {
        assertEquals("0 of your own", cardioReading(0))
        assertEquals("1 of your own", cardioReading(1))
        assertEquals("4 of your own", cardioReading(4))
    }
}
