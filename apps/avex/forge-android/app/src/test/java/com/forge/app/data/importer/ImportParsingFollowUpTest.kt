package com.forge.app.data.importer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.ZoneId

/** Follow-ups to the 2026-09-26 import fixes: two-digit years, dotted dates, and cardio names. */
class ImportParsingFollowUpTest {

    private fun local(y: Int, m: Int, d: Int, h: Int = 0, min: Int = 0) =
        LocalDateTime.of(y, m, d, h, min).atZone(ZoneId.systemDefault()).toInstant().toEpochMilli()

    @Test
    fun twoDigitYear_readsAsThePastNotTheNextCentury() {
        // "5/4/99" used to land in 2099.
        assertEquals(local(1999, 5, 4), ImportParsing.parseEpochMillis("5/4/99", monthFirst = true))
        val thisYear = LocalDate.now().year % 100
        assertEquals(
            local(2000 + thisYear, 5, 4),
            ImportParsing.parseEpochMillis("5/4/%02d".format(thisYear), monthFirst = true)
        )
    }

    @Test
    fun dottedDates_areDayFirstWhateverTheSlashOrder() {
        assertEquals(local(2024, 4, 5), ImportParsing.parseEpochMillis("05.04.2024", monthFirst = true))
        assertEquals(local(2024, 4, 5, 18, 30), ImportParsing.parseEpochMillis("5.4.2024 18:30", monthFirst = false))
    }

    @Test
    fun twelveHourTimeWithNoSpace() {
        assertEquals(local(2024, 5, 4, 18, 30), ImportParsing.parseEpochMillis("5/4/2024 6:30PM", monthFirst = true))
    }

    @Test
    fun cardioMachinesWithNoActivityOfTheirOwn_importAsCardio() {
        assertEquals("other", ImportParsing.cardioRowType("Stair Machine", null, null, 600, null))
        assertEquals("other", ImportParsing.cardioRowType("Jump Rope", null, null, 300, null))
    }

    @Test
    fun timedCarriesAndLunges_stayHolds() {
        assertNull(ImportParsing.cardioRowType("Walking Lunge", null, null, 60, null))
        assertNull(ImportParsing.cardioRowType("Farmer's Walk", null, null, 45, null))
        assertEquals("walk", ImportParsing.cardioRowType("Walking", null, null, 1800, null))
    }
}
