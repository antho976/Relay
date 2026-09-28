package com.forge.app.domain.schedule

import org.junit.Assert.assertEquals
import org.junit.Test

/** Fixed weekdays from onboarding, and keeping them across a regenerate. */
class WeeklyScheduleRemapTest {

    private val four = listOf("upper-a", "lower-a", "upper-b", "lower-b")

    @Test
    fun pickedWeekdaysTakeTheDaysInOrder() {
        // Mon, Tue, Thu, Sat, given out of order.
        val s = WeeklySchedule.fromWeekdays(setOf(5, 0, 3, 1), four)
        assertEquals(listOf("upper-a", "lower-a", "", "upper-b", "", "lower-b", ""), s)
    }

    @Test
    fun spareWeekdaysStayRest() {
        val s = WeeklySchedule.fromWeekdays(listOf(0, 2, 4, 6), listOf("fb-a", "fb-b"))
        assertEquals(listOf("fb-a", "", "fb-b", "", "", "", ""), s)
    }

    @Test
    fun aScheduleThatStillResolvesIsKeptExactly() {
        // Same workout twice a week and a deliberately unscheduled day both survive.
        val old = listOf("upper-a", "", "lower-a", "", "upper-a", "", "")
        assertEquals(old, WeeklySchedule.remap(old, four))
    }

    @Test
    fun aNewSplitWithTheSameCountKeepsTheWeekdays() {
        val old = listOf("push", "pull", "", "legs", "upper", "", "")
        val s = WeeklySchedule.remap(old, four)
        assertEquals(listOf("upper-a", "lower-a", "", "upper-b", "lower-b", "", ""), s)
    }

    @Test
    fun aDifferentCountFallsBackToTheDefaultSpread() {
        val old = listOf("fb-a", "", "fb-b", "", "fb-c", "", "")
        assertEquals(WeeklySchedule.defaultFor(four), WeeklySchedule.remap(old, four))
    }

    @Test
    fun weekdayOfFindsTheDay() {
        val s = WeeklySchedule.fromWeekdays(setOf(0, 1, 3, 4), four)
        assertEquals(3, WeeklySchedule.weekdayOf(s, "upper-b"))
        assertEquals(null, WeeklySchedule.weekdayOf(s, "nope"))
    }
}
