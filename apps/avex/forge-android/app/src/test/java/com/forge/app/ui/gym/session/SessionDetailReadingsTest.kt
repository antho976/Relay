package com.forge.app.ui.gym.session

import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.gym.session.state.ExerciseDetail
import com.forge.app.ui.gym.session.state.SessionMetric
import com.forge.app.ui.gym.session.state.SetDetail
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The session page's derived readings: minute ticks, the top-set change, each metric's row value. */
class SessionDetailReadingsTest {

    @Test
    fun minuteTicksEndOnTheSessionsLength() {
        assertEquals(listOf(0, 10, 20, 30, 40, 48), minuteTicks(48))
        assertEquals(listOf(0, 10, 20, 30, 40, 50), minuteTicks(50))
        assertEquals(listOf(0, 10, 20, 30, 40, 54), minuteTicks(54))
        assertEquals(listOf(0, 15, 30, 45, 60, 75, 90), minuteTicks(90))
        assertEquals(listOf(0), minuteTicks(0))
    }

    private fun set(lb: Double, reps: Int, rpe: Double?) =
        SetDetail(1, lb, "$lb", reps, rpe, false, false, false, false, null, null)

    private fun lift(prev: Double?, sets: List<SetDetail> = listOf(set(195.0, 5, 8.0), set(185.0, 7, null))) =
        ExerciseDetail("Bench", false, null, true, null, sets.maxOf { it.weightLb!! }, sets.sumOf { it.reps },
            sets.sumOf { it.volumeLb }, null, false, sets, prevTopWeightLb = prev)

    @Test
    fun theTopSetComparesWithLastTime() {
        assertEquals("Top set +5 lb" to true, topSetChange(lift(190.0), WeightUnit.LB))
        assertEquals("Top set same" to false, topSetChange(lift(195.0), WeightUnit.LB))
        assertNull(topSetChange(lift(null), WeightUnit.LB))
    }

    @Test
    fun eachMetricReadsItsOwnValue() {
        val ex = lift(null)
        assertEquals("195 lb", liftValue(ex, SessionMetric.WEIGHT, WeightUnit.LB))
        assertEquals("2,270", liftValue(ex, SessionMetric.VOLUME, WeightUnit.LB))
        assertEquals("12", liftValue(ex, SessionMetric.REPS, WeightUnit.LB))
        assertEquals("8.0", liftValue(ex, SessionMetric.RPE, WeightUnit.LB))
        assertEquals("Effort", metricTabLabel(SessionMetric.RPE))
    }
}
