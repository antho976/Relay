package com.forge.app.ui.goals

import com.forge.app.data.repo.GoalRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.domain.units.WeightUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate
import java.time.ZoneOffset
import java.util.Locale

class GoalTargetTest {

    private val day = 86_400_000L

    @Test
    fun liftScaleStartsAtYourBestAndSnapsToPlates() {
        val s = liftScale(205.0, null, WeightUnit.LB)
        assertEquals(205.0, s.lo, 1e-9)
        assertEquals(265.0, s.hi, 1e-9)
        assertEquals(235.0, s.mid, 1e-9)
        assertEquals(225.0, liftDefaultTarget(s), 1e-9)
        assertEquals(230.0, s.snap(228.0), 1e-9)
        assertEquals(265.0, s.at(1.2f), 1e-9)
    }

    @Test
    fun anExistingTargetPastTheSpanWidensIt() {
        val s = liftScale(100.0, 200.0, WeightUnit.KG)
        assertEquals(100.0, s.lo, 1e-9)
        assertEquals(200.0, s.hi, 1e-9)
    }

    @Test
    fun sessionsAWeekStopAtSeven() {
        val s = cumulativeScale(GoalMetric.SESSIONS, GoalPeriod.WEEK, listOf(4.0, 5.0, 9.0), null, WeightUnit.LB)
        assertEquals(1.0, s.lo, 1e-9)
        assertEquals(7.0, s.hi, 1e-9)
        // A step past the average of the finished weeks (4.5) rounds to 6.
        assertEquals(6.0, cumulativeDefaultTarget(GoalMetric.SESSIONS, GoalPeriod.WEEK, listOf(4.0, 5.0, 2.0), s, WeightUnit.LB), 1e-9)
    }

    @Test
    fun landingIsNamedByPartOfMonth() {
        val today = LocalDate.of(2026, 10, 21)
        assertEquals("Early December", landingText(today, 6.0, Locale.US))
        assertEquals("Mid February 2027", landingText(today, 17.0, Locale.US))
    }

    @Test
    fun paceBarsAreDoneThenProjectedToTheTarget() {
        val now = LocalDate.of(2026, 10, 21).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()
        val sets = listOf(80 to 185.0, 50 to 190.0, 20 to 200.0, 2 to 205.0).map { (ago, w) -> GoalRepository.TopSet(now - ago * day, w) }
        val bars = paceBars(sets, now, 225.0, perWeekLb = 5.0, locale = Locale.US, zone = ZoneOffset.UTC)
        val done = bars.filter { !it.projected }
        assertEquals(6, done.size)
        assertEquals("Now", done.last().label)
        assertEquals(205.0, done.last().valueLb, 1e-9)
        // 10 lb a fortnight: 215, then 225 (the target), and it stops there.
        assertEquals(listOf(215.0, 225.0), bars.filter { it.projected }.map { it.valueLb })
        assertTrue(liftPerWeek(sets, now) > 0)
    }

    @Test
    fun pinCaptionWarnsWhenHomeIsFull() {
        assertEquals("Shows in Home's goals (3 of 3 used)", pinCaption(pinned = true, otherPins = 2))
        assertEquals("Home has 3 of 3 · pinning drops the oldest", pinCaption(pinned = false, otherPins = 3))
    }
}
