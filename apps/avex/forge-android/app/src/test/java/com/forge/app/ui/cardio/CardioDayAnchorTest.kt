package com.forge.app.ui.cardio

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.ZoneId

/**
 * M-15: the Cardio week is anchored on a DAY the screen is told about, not on a clock read inside
 * whichever pass happens to run.
 *
 * The signal used to be reduced to the week's Monday before anything downstream saw it, so every
 * day boundary from Tuesday to Sunday was an equal value that a `MutableStateFlow` drops: the
 * Mon–Sun cells kept styling Monday as today, the streak kept counting to yesterday, and none of it
 * recovered while the screen stayed open. Passing the day in is what makes the transition
 * expressible here at all — the old helpers read `LocalDate.now()` and could only be tested on the
 * day the suite happened to run.
 */
class CardioDayAnchorTest {

    private val zone = ZoneId.of("UTC")

    private fun ms(year: Int, month: Int, day: Int, hour: Int = 12): Long =
        LocalDateTime.of(year, month, day, hour, 0).atZone(zone).toInstant().toEpochMilli()

    private fun run(dateMs: Long, dur: Int = 30) =
        CardioEntry(date = dateMs, type = CardioType.RUN.code, durationMin = dur)

    // Monday 2026-06-22 … Sunday 2026-06-28.
    private val monday = LocalDate.of(2026, 6, 22)

    @Test
    fun futureDaysStayEmptyAndTheDayItselfDecidesWhichAreFuture() {
        val week = listOf(run(ms(2026, 6, 22), dur = 40), run(ms(2026, 6, 23), dur = 25))

        val onMonday = CardioViewModel.buildWeekDays(week, zone, monday)
        assertEquals(40, onMonday[0].minutes)
        assertEquals("Tuesday has not happened yet on Monday", 0, onMonday[1].minutes)

        val onTuesday = CardioViewModel.buildWeekDays(week, zone, monday.plusDays(1))
        assertEquals(40, onTuesday[0].minutes)
        assertEquals("and it has by Tuesday — the same entries, a different day", 25, onTuesday[1].minutes)
    }

    @Test
    fun crossingMidnightChangesTheCellsWithoutAnyNewEntry() {
        val week = listOf(run(ms(2026, 6, 22)))

        val before = CardioViewModel.buildWeekDays(week, zone, monday)
        val after = CardioViewModel.buildWeekDays(week, zone, monday.plusDays(1))

        assertTrue("the week row must not be identical across a day boundary", before != after)
    }

    @Test
    fun theStreakEndsOnTheAnchorDayNotOnWhateverTheClockSays() {
        val entries = listOf(run(ms(2026, 6, 22)), run(ms(2026, 6, 23)))

        assertEquals("counted on Tuesday: Monday and Tuesday", 2, CardioViewModel.computeCardioStreak(entries, zone, monday.plusDays(1)))
        assertEquals("counted on Wednesday: still live, ending yesterday", 2, CardioViewModel.computeCardioStreak(entries, zone, monday.plusDays(2)))
        assertEquals("counted on Thursday: the gap has broken it", 0, CardioViewModel.computeCardioStreak(entries, zone, monday.plusDays(3)))
    }

    @Test
    fun aRestOnlyDayIsMarkedRestAndAnActiveOneIsNot() {
        val week = listOf(
            CardioEntry(date = ms(2026, 6, 22), type = CardioType.REST.code, durationMin = 0),
            run(ms(2026, 6, 23), dur = 30)
        )
        val cells = CardioViewModel.buildWeekDays(week, zone, monday.plusDays(1))

        assertTrue(cells[0].isRest)
        assertEquals(0, cells[0].minutes)
        assertEquals(30, cells[1].minutes)
        assertTrue(!cells[1].isRest)
    }
}
