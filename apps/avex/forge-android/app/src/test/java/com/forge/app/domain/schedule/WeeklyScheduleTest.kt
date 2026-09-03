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
        assertEquals(
            WeeklySchedule.NextUp("lower-a", 0, WeeklySchedule.Placement.TODAY),
            withOffset(todayIndex = 0)
        )
    }

    @Test fun weekday_restDayNamesTheNextWorkoutWithItsDistance() {
        // Wednesday is blank: Thursday's Upper B is next, but it is tomorrow's session, not today's.
        assertEquals(
            WeeklySchedule.NextUp("upper-b", 1, WeeklySchedule.Placement.UPCOMING),
            withOffset(todayIndex = 2)
        )
        // Saturday: Monday's legs are two days out.
        assertEquals(
            WeeklySchedule.NextUp("lower-a", 2, WeeklySchedule.Placement.UPCOMING),
            withOffset(todayIndex = 5)
        )
    }

    @Test fun weekday_alreadyTrainedToday_rollsForwardWithADistance() {
        assertEquals(
            WeeklySchedule.NextUp("upper-a", 1, WeeklySchedule.Placement.UPCOMING),
            withOffset(todayIndex = 0, trainedToday = setOf("lower-a"))
        )
    }

    @Test fun sequenceMode_hasNoCalendar_soItsAnswerIsToday() {
        assertEquals(
            WeeklySchedule.NextUp("lower-a", 0, WeeklySchedule.Placement.TODAY),
            WeeklySchedule.resolveNextUpWithOffset(
                WeeklySchedule.MODE_SEQUENCE, 2, schedule, keys, "upper-a", emptySet()
            )
        )
    }

    /**
     * A weekday schedule that resolves nothing is NOT a schedule that says "train today".
     *
     * The sequence fallback reported offset zero, which is the same value a genuinely scheduled
     * session carries, so an all-rest week — or one whose every slot names a day a regenerate has
     * since dropped — was announced as today's scheduled training. The day is still offered, because
     * a user is never left without a suggestion; it is offered as a suggestion.
     */
    @Test fun weekday_withNothingResolvable_isUnscheduledRatherThanToday() {
        assertEquals(
            WeeklySchedule.NextUp("upper-b", 0, WeeklySchedule.Placement.UNSCHEDULED),
            WeeklySchedule.resolveNextUpWithOffset(
                WeeklySchedule.MODE_WEEKDAY, 0, List(7) { "" }, keys, "lower-a", emptySet()
            )
        )
        // Every slot filled, every key stale — a program regenerated out from under the schedule.
        assertEquals(
            WeeklySchedule.NextUp("upper-b", 0, WeeklySchedule.Placement.UNSCHEDULED),
            WeeklySchedule.resolveNextUpWithOffset(
                WeeklySchedule.MODE_WEEKDAY, 0, List(7) { "gone-$it" }, keys, "lower-a", emptySet()
            )
        )
    }

    // ── What the Today directive is actually handed ──────────────────────────

    /**
     * The directive's two inputs, over the states that produce them. This is the contract
     * `DirectiveRepository` reads, and the row it used to get wrong is the last one: an
     * unresolvable weekday schedule handed a day to OPEN, not a day to suggest.
     */
    @Test fun theDirectiveIsOnlyGivenADayToOpenWhenTheScheduleSaysToday() {
        val today = withOffset(todayIndex = 0)
        assertEquals("lower-a", WeeklySchedule.trainTodayKey(today))
        assertNull(WeeklySchedule.upcomingKey(today))

        // Wednesday: a deliberate rest day that knows what is coming.
        val rest = withOffset(todayIndex = 2)
        assertNull(WeeklySchedule.trainTodayKey(rest))
        assertEquals("upper-b", WeeklySchedule.upcomingKey(rest))

        // Every slot rest: a suggestion, and not a session to open today.
        val allRest = WeeklySchedule.resolveNextUpWithOffset(
            WeeklySchedule.MODE_WEEKDAY, 0, List(7) { "" }, keys, "lower-a", emptySet()
        )
        assertNull("an all-rest week is a rest day, not today's training",
            WeeklySchedule.trainTodayKey(allRest))
        assertNull("and nothing is dated, so nothing is announced as coming",
            WeeklySchedule.upcomingKey(allRest))

        // Every slot naming a day the program no longer has — the same state, arrived at by a
        // regenerate rather than by the user.
        val allStale = WeeklySchedule.resolveNextUpWithOffset(
            WeeklySchedule.MODE_WEEKDAY, 3, List(7) { "gone-$it" }, keys, "lower-a", emptySet()
        )
        assertNull(WeeklySchedule.trainTodayKey(allStale))
        assertNull(WeeklySchedule.upcomingKey(allStale))

        // Sequence mode has no calendar, so its answer IS today's.
        val sequence = WeeklySchedule.resolveNextUpWithOffset(
            WeeklySchedule.MODE_SEQUENCE, 2, schedule, keys, "upper-a", emptySet()
        )
        assertEquals("lower-a", WeeklySchedule.trainTodayKey(sequence))

        assertNull(WeeklySchedule.trainTodayKey(null))
        assertNull(WeeklySchedule.upcomingKey(null))
    }

    @Test fun keyOnlyResolverAgreesWithTheOffsetResolver() {
        for (i in 0 until 7) assertEquals(withOffset(i)?.dayKey, weekday(todayIndex = i))
    }
}
