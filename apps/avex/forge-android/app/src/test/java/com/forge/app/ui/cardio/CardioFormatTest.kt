package com.forge.app.ui.cardio

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioWeekPoint
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.time.LocalDate
import java.util.Locale

class CardioFormatTest {

    @Test
    fun aRunReadsAsPaceAndARideAsSpeed() {
        val run = CardioEntry(1, 0, "run", 40, 4.2 * 1.609344)
        assertEquals("9:31" to " /mi", sessionFigure(run, useMiles = true))
        val ride = CardioEntry(2, 0, "cycle", 30, 7.1 * 1.609344)
        assertEquals("14.2" to " mph", sessionFigure(ride, useMiles = true))
        assertNull(sessionFigure(CardioEntry(3, 0, "yoga", 30, null), useMiles = true))
    }

    @Test
    fun distanceAgainstLastWeek() {
        assertEquals("↑ 1.2 vs last" to true, distanceVsLast(10.2, 9.0, useMiles = false))
        assertEquals("↓ 2.0 vs last" to true, distanceVsLast(3.0, 5.0, useMiles = false))
        assertEquals("Same as last" to false, distanceVsLast(5.0, 5.01, useMiles = false))
    }

    @Test
    fun meterCaptionNamesWhatIsLeft() {
        assertEquals("55 min to go · the WHO weekly mark", weekMeterCaption(95, 150, personal = false))
        assertEquals("200 reached · your weekly target", weekMeterCaption(210, 200, personal = true))
    }

    @Test
    fun weekRange() {
        assertEquals("Oct 19 to 25", weekRangeLabel(LocalDate.of(2026, 10, 19), LocalDate.of(2026, 10, 25), Locale.US))
        assertEquals("Sep 28 to Oct 4", weekRangeLabel(LocalDate.of(2026, 9, 28), LocalDate.of(2026, 10, 4), Locale.US))
    }

    @Test
    fun hourTicksAndOrdinals() {
        assertEquals("6a", hourTick(6, false))
        assertEquals("12p", hourTick(12, false))
        assertEquals("15", hourTick(15, true))
        assertEquals("4th", ordinal(4))
        assertEquals("11th", ordinal(11))
        assertEquals("22nd", ordinal(22))
    }

    private fun week(start: Long, min: Int) = CardioWeekPoint(start, min, 0.0, 1, 1, List(7) { 0 })

    @Test
    fun weeksReadJudgesOnlyLivedFinishedWeeks() {
        val w = (0L..7L).map { week(it, listOf(160, 175, 120, 155, 90, 165, 170, 95)[it.toInt()]) }
        val before = (100L..107L).map { week(it, 120) }
        val read = weeksRead(w, before, 150, currentWeekStartMs = 7L, firstLoggedWeekMs = 0L)
        assertEquals(7, read.judged)
        assertEquals(5, read.onTarget)
        assertEquals(147, read.averageMin)
        assertEquals(23, read.changePct)
        assertEquals("Running" to false, weekStatus(w[7], 150, 7L, 0L))
        assertEquals("Short 60" to false, weekStatus(w[4], 150, 7L, 0L))
        assertEquals("On target" to true, weekStatus(w[0], 150, 7L, 0L))
    }
}
