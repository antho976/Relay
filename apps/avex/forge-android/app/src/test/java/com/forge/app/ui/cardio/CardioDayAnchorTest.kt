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

    /**
     * Crossing midnight changes the row, and this pins BOTH of the ways it does.
     *
     * The cells first. [CardioDayCell] carries minutes and a rest flag and nothing else, so a
     * FUTURE day and a past day with nothing logged are the same value — comparing cell lists over
     * a week whose only entry is Monday's compares two identical lists and always has. The version
     * of this test that did exactly that reported a defect in the day anchor that was not there,
     * for the whole of its life. Tuesday needs an entry for the cells to be able to differ at all.
     */
    @Test
    fun crossingMidnightRevealsTheNewDaysOwnEntry() {
        val week = listOf(run(ms(2026, 6, 22), dur = 40), run(ms(2026, 6, 23), dur = 25))

        val before = CardioViewModel.buildWeekDays(week, zone, monday)
        val after = CardioViewModel.buildWeekDays(week, zone, monday.plusDays(1))

        assertTrue("the week row must not be identical across a day boundary", before != after)
        assertEquals("Tuesday is still ahead on Monday", 0, before[1].minutes)
        assertEquals("and is its own logged day by Tuesday", 25, after[1].minutes)
    }

    /**
     * ...and the today styling, which is the half that moves even on a day with nothing logged.
     * `WeekBoxRow` takes `todayDow` as its own input, derived from the same anchor, so the dashed
     * marker and the bold weekday letter follow midnight whether or not any cell value changes.
     */
    @Test
    fun theTodayMarkerFollowsTheAnchorEvenWithNothingLogged() {
        fun todayDow(on: LocalDate) = on.dayOfWeek.value - 1

        assertEquals(0, todayDow(monday))
        assertEquals("a day later marks a different column, with no entry involved", 1, todayDow(monday.plusDays(1)))

        // And the cells behind it are genuinely unchanged, which is why the marker has to be its
        // own input rather than something the cell list could imply.
        val week = listOf(run(ms(2026, 6, 22)))
        assertEquals(
            CardioViewModel.buildWeekDays(week, zone, monday.plusDays(1)),
            CardioViewModel.buildWeekDays(week, zone, monday.plusDays(2))
        )
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
