package com.forge.app.domain.schedule

import com.forge.app.domain.schedule.WeekPlan.Kind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class WeekPlanTest {

    private val upperLower = listOf("upper-a", "lower-a", "", "upper-b", "lower-b", "", "")
    private val keys = setOf("upper-a", "lower-a", "upper-b", "lower-b")
    private val lower = setOf("lower-a", "lower-b")

    private fun plan(
        schedule: List<String> = upperLower,
        valid: Set<String> = keys,
        lowerKeys: Set<String> = lower,
        target: Int = 150,
        allowHard: Boolean = true
    ) = WeekPlan.layout(WeekPlan.Inputs(schedule, valid, lowerKeys, target, allowHard))

    private fun List<WeekPlan.Day>.kinds() = map { it.kind }

    @Test
    fun `lifting days stay on their weekdays`() {
        val week = plan()
        assertEquals(listOf("upper-a", "lower-a", null, "upper-b", "lower-b", null, null), week.map { it.dayKey })
        assertEquals(listOf(0, 1, 3, 4), week.filter { it.kind == Kind.LIFT }.map { it.weekday })
    }

    @Test
    fun `one free day always stays a rest day`() {
        // Four lifting days leave three free; a 150 minute target wants four sessions and gets two.
        val week = plan()
        assertEquals(2, week.count { it.kind == Kind.CARDIO })
        assertEquals(1, week.count { it.kind == Kind.REST })
    }

    @Test
    fun `a single free day is rest, never cardio`() {
        val six = listOf("a", "b", "c", "d", "e", "f", "")
        val week = plan(schedule = six, valid = six.filter { it.isNotEmpty() }.toSet(), lowerKeys = setOf("c", "f"))
        assertEquals(Kind.REST, week[6].kind)
        assertEquals(0, week.count { it.kind == Kind.CARDIO })
    }

    @Test
    fun `no target plans no cardio and leaves the free days as rest`() {
        val week = plan(target = 0)
        assertEquals(0, week.count { it.kind == Kind.CARDIO })
        assertEquals(3, week.count { it.kind == Kind.REST })
        assertTrue(week.all { it.cardioMinutes == 0 })
    }

    @Test
    fun `the hard session never sits the day before a lower body day`() {
        val week = plan()
        val hard = week.single { it.hard }
        assertFalse((hard.weekday + 1) % 7 in listOf(1, 4))
        assertEquals(WeekPlan.HARD_SESSION, hard.cardioMinutes)
    }

    @Test
    fun `with no base there is no hard session`() {
        assertTrue(plan(allowHard = false).none { it.hard })
    }

    @Test
    fun `no hard session when every cardio day is followed by legs`() {
        // Full body three days: Mon, Wed, Fri all train legs. Free: Tue, Thu, Sat, Sun. Two sessions
        // land on Thursday and Sunday, each the day before a lift, so neither may be the hard one.
        val fb = listOf("fb-a", "", "fb-b", "", "fb-c", "", "")
        val all = setOf("fb-a", "fb-b", "fb-c")
        val week = plan(schedule = fb, valid = all, lowerKeys = all, target = 60)
        assertEquals(listOf(3, 6), week.filter { it.kind == Kind.CARDIO }.map { it.weekday })
        assertTrue(week.none { it.hard })
    }

    @Test
    fun `cardio days are spread apart rather than bunched`() {
        // Two lifting days, five free; three sessions wanted for a 100 minute target.
        val two = listOf("a", "", "", "b", "", "", "")
        val week = plan(schedule = two, valid = setOf("a", "b"), lowerKeys = emptySet(), target = 100)
        val cardio = week.filter { it.kind == Kind.CARDIO }.map { it.weekday }
        assertEquals(3, cardio.size)
        // No two of the three on consecutive days.
        assertTrue(cardio.zipWithNext().all { (a, b) -> b - a >= 2 })
    }

    @Test
    fun `what does not fit becomes an easy finisher on an upper day, never after legs`() {
        val week = plan()
        val finishers = week.filter { it.kind == Kind.LIFT && it.cardioMinutes > 0 }
        assertTrue(finishers.isNotEmpty())
        assertTrue(finishers.all { it.dayKey !in lower })
        assertTrue(finishers.all { it.cardioMinutes in WeekPlan.MIN_FINISHER..WeekPlan.MAX_FINISHER })
        assertTrue(week.filter { it.dayKey in lower }.all { it.cardioMinutes == 0 })
    }

    @Test
    fun `the week never plans more than the target`() {
        for (target in listOf(30, 60, 90, 150, 200, 300)) {
            assertTrue("target $target", plan(target = target).sumOf { it.cardioMinutes } <= target)
        }
    }

    @Test
    fun `a slot naming a day the program no longer has is not a lifting day`() {
        val stale = listOf("gone", "lower-a", "", "upper-b", "lower-b", "", "")
        val week = plan(schedule = stale)
        assertTrue(week[0].kind != Kind.LIFT)
    }

    @Test
    fun `the same inputs always give the same week`() {
        assertEquals(plan(), plan())
    }

    @Test
    fun `a missed cardio day rolls into the ones left`() {
        val week = plan(allowHard = false)
        val days = week.filter { it.kind == Kind.CARDIO }.map { it.weekday }
        val planned = week.first { it.kind == Kind.CARDIO }.cardioMinutes
        // On plan so far: the last cardio day asks for its own minutes.
        val before = week.filter { it.weekday < days.last() }.sumOf { it.cardioMinutes }
        assertEquals(planned, WeekPlan.minutesToday(week, days.last(), 150, doneMinutes = before, doneToday = false))
        // Nothing done all week: it carries what the earlier days missed, up to the cap.
        assertEquals(WeekPlan.MAX_SESSION, WeekPlan.minutesToday(week, days.last(), 150, doneMinutes = 0, doneToday = false))
    }

    @Test
    fun `a rolled session never asks for more than the week still owes`() {
        val week = plan(allowHard = false)
        val last = week.last { it.kind == Kind.CARDIO }.weekday
        // 130 of 150 done elsewhere: 20 is all that is left to ask for.
        assertEquals(20, WeekPlan.minutesToday(week, last, 150, doneMinutes = 130, doneToday = false))
    }

    @Test
    fun `a week at its target asks for nothing more`() {
        val week = plan()
        val day = week.first { it.kind == Kind.CARDIO }.weekday
        assertEquals(0, WeekPlan.minutesToday(week, day, targetMinutes = 150, doneMinutes = 150, doneToday = false))
    }

    @Test
    fun `cardio already done today is not asked for twice`() {
        val week = plan()
        val day = week.first { it.kind == Kind.CARDIO }.weekday
        assertEquals(0, WeekPlan.minutesToday(week, day, 150, 20, doneToday = true))
    }

    @Test
    fun `a rest day asks for nothing`() {
        val week = plan()
        val rest = week.first { it.kind == Kind.REST }.weekday
        assertEquals(0, WeekPlan.minutesToday(week, rest, 150, 0, false))
    }
}
