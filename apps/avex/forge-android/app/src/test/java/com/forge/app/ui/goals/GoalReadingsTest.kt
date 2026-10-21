package com.forge.app.ui.goals

import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.domain.units.WeightUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate
import java.time.ZoneOffset
import java.util.Locale

class GoalReadingsTest {

    private val day = 86_400_000L

    @Test
    fun etaIsRemainingOverPaceAndNullWhenNotHeadingThere() {
        assertEquals(6.0, etaWeeks(12.0, 2.0)!!, 1e-9)
        assertEquals(0.0, etaWeeks(-1.0, 0.0)!!, 1e-9)
        assertNull(etaWeeks(10.0, 0.0))
        assertEquals("~6 weeks", etaText(6.2))
        assertEquals("~1 week", etaText(0.4))
        assertEquals("~4 mo", etaText(17.0))
    }

    @Test
    fun changeLineNamesDirection() {
        assertEquals("↑ 10 in 30d", changeLine(10.0))
        assertEquals("↓ 1.4 in 30d", changeLine(-1.4))
        assertEquals("Steady over 30d", changeLine(0.0))
    }

    @Test
    fun liftLineReadsPaceFromThirtyDayChange() {
        val g = GoalRepository.GoalProgress("bench", "Bench press", 225.0, 205.0, 0L)
        val line = liftLine(g, change30d = 30.0, reachedAtMs = null, unit = WeightUnit.LB, locale = Locale.US, zone = ZoneOffset.UTC)
        // 30 lb a month is 7 lb a week; 20 to go is about 3 weeks.
        assertEquals("~3 weeks", line.paceTop)
        assertTrue(line.paceTopMoved)
        assertEquals("205", line.figure)
        assertEquals(" / 225 lb", line.denominator)
        assertEquals("↑ 30 in 30d", line.paceBottom)
    }

    @Test
    fun aStalledLiftSaysWhatIsLeftNotAnEta() {
        val g = GoalRepository.GoalProgress("bench", "Bench press", 225.0, 205.0, 0L)
        val line = liftLine(g, change30d = null, reachedAtMs = null, unit = WeightUnit.LB, locale = Locale.US, zone = ZoneOffset.UTC)
        assertEquals("20 lb to go", line.paceTop)
        assertFalse(line.paceTopMoved)
        assertNull(line.paceBottom)
    }

    @Test
    fun aSlowPaceIsNamedSlow() {
        val g = GoalRepository.GoalProgress("dl", "Deadlift", 405.0, 305.0, 0L)
        val line = liftLine(g, change30d = 10.0, reachedAtMs = null, unit = WeightUnit.LB, locale = Locale.US, zone = ZoneOffset.UTC)
        assertTrue(line.paceTop.startsWith("Slow · "))
        assertFalse(line.paceTopMoved)
    }

    @Test
    fun aReachedLiftShowsWhenItWasMet() {
        val g = GoalRepository.GoalProgress("squat", "Squat", 275.0, 280.0, 0L)
        val oct16 = LocalDate.of(2026, 10, 16).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()
        val line = liftLine(g, null, oct16, WeightUnit.LB, Locale.US, ZoneOffset.UTC)
        assertTrue(line.achieved)
        assertEquals("Squat 275 lb", line.reachedTitle)
        assertEquals("Oct 16", line.reachedWhen)
    }

    @Test
    fun firstReachedAtIsTheFirstSessionAtTarget() {
        val sets = listOf(GoalRepository.TopSet(3, 280.0), GoalRepository.TopSet(1, 260.0), GoalRepository.TopSet(2, 275.0))
        assertEquals(2L, firstReachedAt(sets, 275.0))
        assertNull(firstReachedAt(sets, 300.0))
    }

    @Test
    fun aWeeklyGoalIsOnPlanWhenItKeepsUpWithTheWeek() {
        // Wednesday: 3 of 7 days gone, 3 of 5 workouts done is ahead of 2.1.
        val wed = LocalDate.of(2026, 10, 21)
        assertEquals(3f / 7f, periodElapsed(GoalPeriod.WEEK, wed), 1e-6f)
        assertTrue(onPlan(3.0, 5.0, periodElapsed(GoalPeriod.WEEK, wed)))
        assertFalse(onPlan(1.0, 5.0, periodElapsed(GoalPeriod.WEEK, wed)))
        val g = ExtendedGoalRepository.Progress(1, GoalMetric.SESSIONS, GoalPeriod.WEEK, 5.0, 3.0, "", 0.6f, false, 0L)
        val line = customLine(g, emptyList(), WeightUnit.LB, false, wed)
        assertEquals("5 days left", line.paceTop)
        assertEquals("On plan", line.paceBottom)
        assertEquals(" / 5 workouts", line.denominator)
    }

    @Test
    fun bodyweightPaceComesFromWeighIns() {
        val now = 100 * day
        val weighIns = listOf(now - 30 * day to 186.4, now - 10 * day to 184.0, now to 182.4)
        assertEquals(-4.0, weighInChange30d(weighIns)!!, 1e-9)
        val g = ExtendedGoalRepository.Progress(2, GoalMetric.BODYWEIGHT, GoalPeriod.ALL, 178.0, 182.4, "", 0.5f, false, 0L, baselineValue = 190.0)
        val line = customLine(g, weighIns, WeightUnit.LB, false, LocalDate.of(2026, 10, 21))
        // 4 lb a month toward the target is about 0.93 a week; 4.4 to go is under 5 weeks.
        assertEquals("~5 weeks", line.paceTop)
        assertEquals("↓ 4 in 30d", line.paceBottom)
    }

    @Test
    fun metaLineLeavesOutZeros() {
        assertEquals("4 active · 3 pinned to Home · 2 reached", goalsMetaLine(4, 3, 2))
        assertEquals("0 active", goalsMetaLine(0, 0, 0))
    }
}
