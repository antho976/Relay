package com.forge.app.domain.coach

import com.forge.app.domain.coach.TodayDirective.Directive
import com.forge.app.domain.coach.TodayDirective.Kind
import com.forge.app.domain.engine.ConditioningPlanner
import com.forge.app.domain.engine.ConditioningProfile
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/** The week plan laid over the lifting answer ([TodayDirective.withWeek]). */
class TodayDirectiveWeekTest {

    private val noZones = ConditioningProfile()
    private val easy = ConditioningPlanner.steady(noZones, 35, phase = null)
    private val hard = ConditioningPlanner.intervals(noZones, weekdayMode = true)
    private val finisher = ConditioningPlanner.finisher(noZones, 12)

    private val dayOff = Directive(Kind.REST, "Rest today", "Today is a rest day in your schedule.", restFromLifting = true)
    private val train = Directive(Kind.TRAIN, "Upper A", "It's what today's schedule calls for.", dayKey = "upper-a", routine = true)
    private val sick = Directive(Kind.REST, "Rest today", "You flagged being unwell.")
    private val done = Directive(Kind.REST, "Done for today", "You've trained.", liftingDone = true)

    @Test
    fun `a day off the bar becomes its planned cardio session`() {
        val d = TodayDirective.withWeek(dayOff, easy, plannedIsSession = true)
        assertEquals(Kind.CARDIO, d.kind)
        assertEquals("35 min easy", d.headline)
        assertEquals("A pace you could hold a conversation at", d.secondary)
        assertSame(easy, d.cardio)
        assertNull(d.dayKey)
    }

    @Test
    fun `zones turn the cue into a bpm band`() {
        val zoned = ConditioningPlanner.steady(ConditioningProfile(ageYears = 30, restingHr = 60), 30, null)
        val d = TodayDirective.withWeek(dayOff, zoned, plannedIsSession = true)
        assertTrue(d.secondary!!.startsWith("Zone 2 · "))
        assertTrue(d.secondary!!.endsWith(" bpm"))
    }

    @Test
    fun `the hard session reads as intervals with its whole recipe`() {
        val d = TodayDirective.withWeek(dayOff, hard, plannedIsSession = true)
        assertEquals("Intervals", d.headline)
        assertEquals("8 x 1 min hard, 1 min easy between · 8 min warm-up · 5 min cool-down", d.secondary)
    }

    @Test
    fun `a day off with nothing planned stays a rest day`() {
        assertSame(dayOff, TodayDirective.withWeek(dayOff, null, plannedIsSession = false))
    }

    @Test
    fun `a lifting day keeps its headline and picks up its finisher`() {
        val d = TodayDirective.withWeek(train, finisher, plannedIsSession = false)
        assertEquals(Kind.TRAIN, d.kind)
        assertEquals("Upper A", d.headline)
        assertEquals("upper-a", d.dayKey)
        assertEquals("Then 12 min easy cardio.", d.secondary)
        assertSame(finisher, d.cardio)
    }

    @Test
    fun `once the lifting is done the finisher is what the day has left`() {
        val d = TodayDirective.withWeek(done, finisher, plannedIsSession = false)
        assertEquals(Kind.CARDIO, d.kind)
        assertEquals("12 min easy", d.headline)
    }

    @Test
    fun `a rest for illness is never overruled by the plan`() {
        assertSame(sick, TodayDirective.withWeek(sick, easy, plannedIsSession = true))
        val seq = TodayDirective.DayByDay(liftedYesterday = true, weekHasRoom = true, cardio = easy, nextDayName = "Upper A")
        assertSame(sick, TodayDirective.withWeek(sick, null, false, seq))
    }

    // ── Sequence mode: no week to read, so it alternates day by day ────────────

    @Test
    fun `the day after a lift is cardio while the week owes any`() {
        val seq = TodayDirective.DayByDay(liftedYesterday = true, weekHasRoom = true, cardio = easy, nextDayName = "Upper A")
        val d = TodayDirective.withWeek(train, null, false, seq)
        assertEquals(Kind.CARDIO, d.kind)
        assertEquals("35 min easy", d.headline)
    }

    @Test
    fun `the day after a lift is rest once the cardio is done`() {
        val seq = TodayDirective.DayByDay(liftedYesterday = true, weekHasRoom = true, cardio = null, nextDayName = "Upper A")
        val d = TodayDirective.withWeek(train, null, false, seq)
        assertEquals(Kind.REST, d.kind)
        assertEquals("Upper A is next.", d.secondary)
        assertTrue(d.restFromLifting)
    }

    @Test
    fun `a week with no room keeps the lifting day`() {
        val seq = TodayDirective.DayByDay(liftedYesterday = true, weekHasRoom = false, cardio = easy, nextDayName = "Upper A")
        assertSame(train, TodayDirective.withWeek(train, null, false, seq))
    }

    @Test
    fun `two days since the last lift is a lifting day`() {
        val seq = TodayDirective.DayByDay(liftedYesterday = false, weekHasRoom = true, cardio = easy, nextDayName = "Upper A")
        assertSame(train, TodayDirective.withWeek(train, null, false, seq))
    }

    @Test
    fun `a comeback or a first session is not traded for cardio`() {
        // Neither is the ordinary "train what's next" answer, so neither carries `routine`.
        val comeback = train.copy(routine = false)
        val seq = TodayDirective.DayByDay(liftedYesterday = true, weekHasRoom = true, cardio = easy, nextDayName = "Upper A")
        assertSame(comeback, TodayDirective.withWeek(comeback, null, false, seq))
    }

    @Test
    fun `the weekly budget being met is a day off like any other`() {
        val seq = TodayDirective.DayByDay(liftedYesterday = true, weekHasRoom = true, cardio = easy, nextDayName = null)
        assertEquals(Kind.CARDIO, TodayDirective.withWeek(dayOff, null, false, seq).kind)
    }
}
