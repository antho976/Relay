package com.forge.app.ui.gym.stats

import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.projections.SetWithExerciseAndSession
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.ZoneOffset

class StatsRangesTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 1_000 * day
    private val utc = ZoneOffset.UTC

    private fun set(daysAgo: Int, weight: Double, reps: Int, id: String = "bench", rpe: Double? = null) =
        SetWithExerciseAndSession(weight, reps, id, now - daysAgo * day, loggedExerciseId = daysAgo.toLong(), rpe = rpe)

    @Test
    fun aSessionIsAPrOnlyWhenNoEarlierSessionReachedIt() {
        val s = liftSessions(listOf(set(30, 100.0, 5), set(20, 105.0, 5), set(10, 100.0, 5), set(5, 110.0, 5)))["bench"]!!
        assertEquals(listOf(true, true, false, true), s.map { it.pr })
    }

    @Test
    fun theChangeRunsFromTheLastSessionBeforeTheRange() {
        val sessions = liftSessions(listOf(set(100, 100.0, 5), set(90, 110.0, 5), set(20, 120.0, 5)))["bench"]!!
        val w = statsWindow(StatsRange.W12, now, null)
        val t = liftTrend("bench", sessions, w)!!
        // One session inside 12 weeks; the change still reads against the one before the range.
        assertEquals(1, t.current.size)
        assertNotNull(t.delta)
        assertTrue(t.delta!! > 0)
        assertTrue(t.prInRange)
    }

    @Test
    fun aFlatLiftSaysForHowLong() {
        val sessions = liftSessions(listOf(set(40, 100.0, 5), set(30, 120.0, 5), set(20, 120.0, 5), set(10, 120.0, 5)))["bench"]!!
        val t = liftTrend("bench", sessions, statsWindow(StatsRange.W4, now, null))!!
        assertNotNull(t.flatWeeks)
        assertEquals("Flat 4 weeks", liftChangeLine(t, StatsRange.W4) { "$it" })
    }

    @Test
    fun versusBeforeComparesTheTwoChanges() {
        assertEquals("8 more than the 12 before", versusBefore(18.0, 10.0, StatsRange.W12) { it.toInt().toString() })
        assertNull(versusBefore(18.0, 10.0, StatsRange.ALL) { "" })
    }

    @Test
    fun effortBucketsByRepsLeft() {
        val sets = listOf(5.0, 7.0, 8.0, 8.5, 9.0, 10.0).mapIndexed { i, rpe -> set(i + 1, 100.0, 5, rpe = rpe) }
        val e = effortRead(sets, now - 30 * day, now)
        // RPE 5 → 5 left (4+), 7 → 3, 8 → 2, 8.5 → 2 (rounds to 2), 9 → 1, 10 → 0.
        assertEquals(listOf(1, 1, 2, 1, 1), e.buckets)
        assertEquals(67, e.inZonePct)
    }

    @Test
    fun weeksRunMondayToMondayAndMarkTheCurrentOne() {
        val rows = listOf(
            SessionDao.SessionVolumeDeloadRow(1, "a", now - 10 * day, 1000.0, false),
            SessionDao.SessionVolumeDeloadRow(2, "a", now - 2 * day, 500.0, true),
        )
        val weeks = weeklyVolumes(rows, statsWindow(StatsRange.W4, now, null), utc)
        assertTrue(weeks.last().current)
        assertEquals(1500.0, weeks.sumOf { it.volumeLb }, 0.01)
        assertFalse(weeks.first().current)
    }

    @Test
    fun monthTicksEndOnNow() {
        val ticks = monthTicks(statsWindow(StatsRange.W12, now, null), utc)
        assertEquals("Now", ticks.last().second)
        assertEquals(1f, ticks.last().first, 0f)
    }
}
