package com.forge.app.domain.schedule

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

/** Planned rest vs missed training, judged by the schedule in force on the day. */
class ScheduleHistoryTest {

    private val weekday = WeeklySchedule.MODE_WEEKDAY
    // Mon/Tue/Thu/Fri.
    private val fourDay = listOf("upper-a", "lower-a", "", "upper-b", "lower-b", "", "")
    // Mon/Wed/Fri.
    private val threeDay = listOf("fb-a", "", "fb-b", "", "fb-c", "", "")

    // 2026-09-07 is a Monday.
    private val monday = LocalDate.of(2026, 9, 7)
    private fun day(offset: Long) = monday.plusDays(offset)

    @Test
    fun beforeTheHistoryNothingIsPlanned() {
        val h = ScheduleHistory.record(emptyList(), day(7).toEpochDay(), weekday, fourDay)
        assertNull(ScheduleHistory.planOn(h, day(2)))
        assertFalse(ScheduleHistory.isPlannedRest(h, day(2)))
    }

    @Test
    fun restAndTrainingDaysFollowTheWeekdays() {
        val h = ScheduleHistory.record(emptyList(), monday.toEpochDay(), weekday, fourDay)
        assertEquals(false, ScheduleHistory.planOn(h, day(0)))  // Mon
        assertEquals(true, ScheduleHistory.planOn(h, day(2)))   // Wed
        assertEquals(true, ScheduleHistory.planOn(h, day(6)))   // Sun
        assertEquals(true, ScheduleHistory.planOn(h, day(9)))   // next Wed, still in force
    }

    @Test
    fun aChangeDoesNotRepaintThePast() {
        var h = ScheduleHistory.record(emptyList(), monday.toEpochDay(), weekday, fourDay)
        h = ScheduleHistory.record(h, day(14).toEpochDay(), weekday, threeDay)
        assertEquals("old Tuesday was a training day", false, ScheduleHistory.planOn(h, day(1)))
        assertEquals("new Tuesday is rest", true, ScheduleHistory.planOn(h, day(15)))
    }

    @Test
    fun sequenceModePlansNoRest() {
        var h = ScheduleHistory.record(emptyList(), monday.toEpochDay(), weekday, fourDay)
        h = ScheduleHistory.record(h, day(7).toEpochDay(), WeeklySchedule.MODE_SEQUENCE, fourDay)
        assertTrue(ScheduleHistory.isPlannedRest(h, day(2)))
        assertNull(ScheduleHistory.planOn(h, day(9)))
    }

    @Test
    fun anEmptyWeekIsNotSevenRestDays() {
        val h = ScheduleHistory.record(emptyList(), monday.toEpochDay(), weekday, List(7) { "" })
        assertNull(ScheduleHistory.planOn(h, day(3)))
    }

    @Test
    fun sameDayChangesCollapseAndNoOpsRecordNothing() {
        var h = ScheduleHistory.record(emptyList(), monday.toEpochDay(), weekday, fourDay)
        h = ScheduleHistory.record(h, monday.toEpochDay(), weekday, threeDay)
        assertEquals(1, h.size)
        assertEquals(threeDay, h.single().slots)
        val again = ScheduleHistory.record(h, day(3).toEpochDay(), weekday, threeDay)
        assertEquals(h, again)
    }

    @Test
    fun roundTrips() {
        var h = ScheduleHistory.record(emptyList(), monday.toEpochDay(), weekday, fourDay)
        h = ScheduleHistory.record(h, day(10).toEpochDay(), weekday, threeDay)
        assertEquals(h, ScheduleHistory.parse(ScheduleHistory.encode(h)))
        assertTrue(ScheduleHistory.parse("not json").isEmpty())
        assertTrue(ScheduleHistory.parse(null).isEmpty())
    }

    @Test
    fun restDaysInARange() {
        val h = ScheduleHistory.record(emptyList(), monday.toEpochDay(), weekday, fourDay)
        val rest = ScheduleHistory.plannedRestDays(h, monday.minusDays(3), day(6))
        assertEquals(setOf(day(2), day(5), day(6)).map { it.toEpochDay() }.toSet(), rest)
    }
}
