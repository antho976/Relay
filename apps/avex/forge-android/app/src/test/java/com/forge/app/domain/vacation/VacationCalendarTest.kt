package com.forge.app.domain.vacation

import com.forge.app.data.db.entities.VacationPeriod
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

class VacationCalendarTest {

    private fun period(start: String, end: String) = VacationPeriod(startDate = start, endDate = end)

    @Test
    fun coversInclusiveRange() {
        val on = VacationCalendar.onVacation(listOf(period("2026-06-10", "2026-06-14")))
        assertTrue(on(LocalDate.parse("2026-06-10")))  // start inclusive
        assertTrue(on(LocalDate.parse("2026-06-12")))  // middle
        assertTrue(on(LocalDate.parse("2026-06-14")))  // end inclusive
        assertFalse(on(LocalDate.parse("2026-06-09"))) // day before
        assertFalse(on(LocalDate.parse("2026-06-15"))) // day after
    }

    @Test
    fun emptyPeriodsCoverNothing() {
        val on = VacationCalendar.onVacation(emptyList())
        assertFalse(on(LocalDate.parse("2026-06-12")))
    }

    @Test
    fun reversedRangeIsNormalized() {
        val on = VacationCalendar.onVacation(listOf(period("2026-06-14", "2026-06-10")))
        assertTrue(on(LocalDate.parse("2026-06-12")))
    }

    @Test
    fun unparseableKeysAreIgnoredNotCrashing() {
        val on = VacationCalendar.onVacation(listOf(period("garbage", "2026-06-14"), period("2026-06-10", "2026-06-14")))
        assertTrue(on(LocalDate.parse("2026-06-12")))
        assertFalse(on(LocalDate.parse("2026-01-01")))
    }

    @Test
    fun multipleRangesAreUnioned() {
        val on = VacationCalendar.onVacation(listOf(period("2026-01-01", "2026-01-03"), period("2026-06-10", "2026-06-12")))
        assertTrue(on(LocalDate.parse("2026-01-02")))
        assertTrue(on(LocalDate.parse("2026-06-11")))
        assertFalse(on(LocalDate.parse("2026-03-15")))
    }
}
