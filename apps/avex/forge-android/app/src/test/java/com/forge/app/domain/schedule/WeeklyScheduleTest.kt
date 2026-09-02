package com.forge.app.domain.schedule

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class WeeklyScheduleTest {

    private val keys = listOf("upper-a", "lower-a", "upper-b", "lower-b")

    // Mon=Lower A (legs), Tue=Upper A, Wed=rest, Thu=Upper B, Fri=Lower B, Sat/Sun=rest.
    private val schedule = listOf("lower-a", "upper-a", "", "upper-b", "lower-b", "", "")

    private fun weekday(mode: String = WeeklySchedule.MODE_WEEKDAY, todayIndex: Int, trainedToday: Set<String> = emptySet()) =
        WeeklySchedule.resolveNextUp(mode, todayIndex, schedule, keys, lastFinishedDayKey = null, trainedTodayKeys = trainedToday)

    @Test fun parseAndEncodeRoundTrip() {
        assertEquals(schedule, WeeklySchedule.parse(WeeklySchedule.encode(schedule)))
    }

    @Test fun parsePadsToSevenSlots() {
        assertEquals(7, WeeklySchedule.parse("upper-a").size)
        assertEquals(7, WeeklySchedule.parse("").size)
    }

    @Test fun defaultMapsProgramDaysOntoFirstWeekdays() {
        assertEquals(listOf("upper-a", "lower-a", "upper-b", "lower-b", "", "", ""), WeeklySchedule.defaultFor(keys))
    }

    @Test fun weekday_todaysScheduledWorkoutIsNextUp() {
        // Monday → legs.
        assertEquals("lower-a", weekday(todayIndex = 0))
    }

    @Test fun weekday_alreadyTrainedToday_rollsToNextScheduledDay() {
        // It's Monday and Legs (lower-a) is already done → roll forward to Tuesday's Upper A.
        assertEquals("upper-a", weekday(todayIndex = 0, trainedToday = setOf("lower-a")))
    }

    @Test fun weekday_restDayShowsNextUpcomingWorkout() {
        // Wednesday is a rest slot → the next scheduled workout is Thursday's Upper B.
        assertEquals("upper-b", weekday(todayIndex = 2))
    }

    @Test fun weekday_missedDayIsNotCarried_calendarAdvances() {
        // Tuesday: even though Monday's legs was never done, today simply shows Tuesday's workout.
        assertEquals("upper-a", weekday(todayIndex = 1))
    }

    @Test fun weekday_wrapsAroundTheWeekend() {
        // Saturday/Sunday rest → wrap to Monday's legs.
        assertEquals("lower-a", weekday(todayIndex = 5))
        assertEquals("lower-a", weekday(todayIndex = 6))
    }

    @Test fun weekday_allRest_fallsBackToSequence() {
        val allRest = List(7) { "" }
        // No scheduled workout anywhere → sequence fallback (day after last finished).
        assertEquals(
            "upper-b",
            WeeklySchedule.resolveNextUp(
                WeeklySchedule.MODE_WEEKDAY, 0, allRest, keys,
                lastFinishedDayKey = "lower-a", trainedTodayKeys = emptySet()
            )
        )
    }

    @Test fun sequence_dayAfterLastFinishedCycling() {
        fun seq(last: String?) = WeeklySchedule.resolveNextUp(
            WeeklySchedule.MODE_SEQUENCE, 0, schedule, keys, last, emptySet()
        )
        assertEquals("upper-a", seq(null))        // nothing finished → first day
        assertEquals("lower-a", seq("upper-a"))   // after upper-a → lower-a
        assertEquals("upper-a", seq("lower-b"))   // wraps after the last day
        assertEquals("upper-a", seq("ghost-day")) // stale key → first day
    }

    @Test fun emptyProgram_returnsNull() {
        assertNull(WeeklySchedule.resolveNextUp(WeeklySchedule.MODE_WEEKDAY, 0, schedule, emptyList(), null, emptySet()))
    }

    // ── The offset: "next up" is not "train today" ───────────────────────────

    private fun withOffset(todayIndex: Int, trainedToday: Set<String> = emptySet()) =
        WeeklySchedule.resolveNextUpWithOffset(
            WeeklySchedule.MODE_WEEKDAY, todayIndex, schedule, keys,
            lastFinishedDayKey = null, trainedTodayKeys = trainedToday
        )

    @Test fun weekday_todaysWorkoutIsZeroDaysAhead() {
        assertEquals(WeeklySchedule.NextUp("lower-a", 0), withOffset(todayIndex = 0))
    }

    @Test fun weekday_restDayNamesTheNextWorkoutWithItsDistance() {
        // Wednesday is blank: Thursday's Upper B is next, but it is tomorrow's session, not today's.
        assertEquals(WeeklySchedule.NextUp("upper-b", 1), withOffset(todayIndex = 2))
        // Saturday: Monday's legs are two days out.
        assertEquals(WeeklySchedule.NextUp("lower-a", 2), withOffset(todayIndex = 5))
    }

    @Test fun weekday_alreadyTrainedToday_rollsForwardWithADistance() {
        assertEquals(
            WeeklySchedule.NextUp("upper-a", 1),
            withOffset(todayIndex = 0, trainedToday = setOf("lower-a"))
        )
    }

    @Test fun sequenceAndFallback_haveNoCalendar_soReportToday() {
        assertEquals(
            WeeklySchedule.NextUp("lower-a", 0),
            WeeklySchedule.resolveNextUpWithOffset(
                WeeklySchedule.MODE_SEQUENCE, 2, schedule, keys, "upper-a", emptySet()
            )
        )
        assertEquals(
            WeeklySchedule.NextUp("upper-b", 0),
            WeeklySchedule.resolveNextUpWithOffset(
                WeeklySchedule.MODE_WEEKDAY, 0, List(7) { "" }, keys, "lower-a", emptySet()
            )
        )
    }

    @Test fun keyOnlyResolverAgreesWithTheOffsetResolver() {
        for (i in 0 until 7) assertEquals(withOffset(i)?.dayKey, weekday(todayIndex = i))
    }
}
