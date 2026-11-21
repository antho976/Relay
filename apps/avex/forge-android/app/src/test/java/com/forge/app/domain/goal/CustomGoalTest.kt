package com.forge.app.domain.goal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** JVM unit tests for the custom-goal parameter encoding and pure progress math (#137). */
class CustomGoalTest {

    @Test
    fun goalTypeRoundTrips() {
        val encoded = encodeGoalType(GoalMetric.CARDIO_DISTANCE, GoalPeriod.WEEK)
        assertEquals("cardio_distance:week", encoded)
        val parsed = parseGoalType(encoded)!!
        assertEquals(GoalMetric.CARDIO_DISTANCE, parsed.metric)
        assertEquals(GoalPeriod.WEEK, parsed.period)
    }

    @Test
    fun parseGoalTypeDefaultsPeriodToAll() {
        // A bare metric (legacy / missing period) still parses, defaulting to all-time.
        assertEquals(GoalPeriod.ALL, parseGoalType("sessions")!!.period)
        assertEquals(GoalMetric.SESSIONS, parseGoalType("sessions")!!.metric)
    }

    @Test
    fun parseGoalTypeRejectsUnknownMetric() {
        assertNull(parseGoalType("bogus:week"))
        assertNull(parseGoalType(""))
    }

    @Test
    fun bodyweightIsTheOnlyNonCumulativeMetric() {
        assertFalse(GoalMetric.BODYWEIGHT.isCumulative)
        assertTrue(GoalMetric.CARDIO_DISTANCE.isCumulative)
        assertTrue(GoalMetric.SESSIONS.isCumulative)
        assertTrue(GoalMetric.VOLUME.isCumulative)
    }

    @Test
    fun cumulativeProgressClampsAndDetectsAchieved() {
        assertEquals(0.5f, GoalProgressMath.cumulativeFraction(2.5, 5.0), 0.0001f)
        assertEquals(1f, GoalProgressMath.cumulativeFraction(8.0, 5.0), 0.0001f) // over target clamps to 1
        assertEquals(0f, GoalProgressMath.cumulativeFraction(3.0, 0.0), 0.0001f) // no divide-by-zero
        assertFalse(GoalProgressMath.cumulativeAchieved(4.9, 5.0))
        assertTrue(GoalProgressMath.cumulativeAchieved(5.0, 5.0))
        assertTrue(GoalProgressMath.cumulativeAchieved(6.0, 5.0))
    }

    @Test
    fun bodyweightCutTracksDownward() {
        // Cut from 200 lb toward 180 lb. At 190 you're halfway; at 180 you've reached it.
        assertEquals(0f, GoalProgressMath.bodyweightFraction(200.0, 200.0, 180.0), 0.0001f)
        assertEquals(0.5f, GoalProgressMath.bodyweightFraction(200.0, 190.0, 180.0), 0.0001f)
        assertEquals(1f, GoalProgressMath.bodyweightFraction(200.0, 180.0, 180.0), 0.0001f)
        assertFalse(GoalProgressMath.bodyweightAchieved(200.0, 190.0, 180.0))
        assertTrue(GoalProgressMath.bodyweightAchieved(200.0, 179.0, 180.0)) // passed the target
    }

    @Test
    fun bodyweightBulkTracksUpwardAndIgnoresWrongDirection() {
        // Bulk from 150 lb toward 170 lb.
        assertEquals(0.5f, GoalProgressMath.bodyweightFraction(150.0, 160.0, 170.0), 0.0001f)
        assertTrue(GoalProgressMath.bodyweightAchieved(150.0, 170.0, 170.0))
        // Moving the wrong way (down when bulking) clamps to 0, never negative.
        assertEquals(0f, GoalProgressMath.bodyweightFraction(150.0, 140.0, 170.0), 0.0001f)
        assertFalse(GoalProgressMath.bodyweightAchieved(150.0, 165.0, 170.0))
    }
}
