package com.forge.app.ui.gym.train.components

import com.forge.app.data.db.entities.LoggedSet
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The "rested m:ss" line between two sets: the rest itself, not the gap between two LOG taps. */
class RestBetweenSetsTest {

    private val t0 = 1_700_000_000_000L
    private fun set(atSec: Int, hold: Int? = null) = LoggedSet(
        loggedExerciseId = 1, setIndex = 0, weightText = "100", reps = if (hold == null) 8 else 0,
        completedAt = t0 + atSec * 1000L, durationSeconds = hold
    )

    @Test
    fun theNextSetsOwnTimeComesOff() {
        // Logged at 0, rested 2:00, did a ~45 s set, logged at 2:45.
        assertEquals(120, restBetweenSeconds(set(0), set(165)))
    }

    @Test
    fun aMeasuredHoldIsUsedInsteadOfTheEstimate() {
        assertEquals(120, restBetweenSeconds(set(0), set(180, hold = 60)))
    }

    @Test
    fun otherWorkInsideTheGapIsNotRest() {
        // A superset: another exercise's set logged at 2:30 between these two.
        assertNull(restBetweenSeconds(set(0), set(400), otherSetTimes = listOf(t0 + 150_000L)))
        // Work outside the gap doesn't matter.
        assertEquals(120, restBetweenSeconds(set(0), set(165), otherSetTimes = listOf(t0 - 60_000L, t0 + 500_000L)))
    }

    @Test
    fun walkingOffIsNotRest() {
        assertNull(restBetweenSeconds(set(0), set(20 * 60)))
    }

    @Test
    fun rowsWithoutRealTimesShowNothing() {
        val seeded = set(0).copy(completedAt = 0L)
        assertNull(restBetweenSeconds(seeded, seeded))
        assertNull(restBetweenSeconds(set(10), set(5)))
    }
}
