package com.forge.app.domain.pr

import com.forge.app.data.db.entities.LoggedSet
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PrDetectorTest {

    private fun set(weightLb: Double?, reps: Int) = LoggedSet(
        loggedExerciseId = 1L,
        setIndex = 0,
        weightText = weightLb?.toString() ?: "BW",
        weightLb = weightLb,
        reps = reps,
        completedAt = 0L
    )

    @Test
    fun emptyHistoryIsAPrForAnyWeightedSet() {
        assertTrue(PrDetector.isPr(emptyList(), newWeightLb = 100.0, newReps = 5))
    }

    @Test
    fun bodyweightSetIsNeverAPr() {
        assertFalse(PrDetector.isPr(listOf(set(50.0, 5)), newWeightLb = null, newReps = 10))
    }

    /** A typed "0" parses to 0.0, not null, so it used to sail past the null guard: with no history
     *  it was an unconditional PR, and with history a 0 x 20 "beat" a 100 x 5. */
    @Test
    fun zeroWeightIsNeverAPr() {
        assertFalse(PrDetector.isPr(emptyList(), newWeightLb = 0.0, newReps = 5))
        assertFalse(PrDetector.isPr(listOf(set(100.0, 5)), newWeightLb = 0.0, newReps = 20))
    }

    @Test
    fun nonPositiveRepsIsNeverAPr() {
        assertFalse(PrDetector.isPr(emptyList(), newWeightLb = 100.0, newReps = 0))
    }

    @Test
    fun heavierThanBestAtSameOrHigherRepsIsAPr() {
        assertTrue(PrDetector.isPr(listOf(set(100.0, 10)), newWeightLb = 105.0, newReps = 8))
    }

    @Test
    fun lighterThanBestAtSameOrHigherRepsIsNotAPr() {
        assertFalse(PrDetector.isPr(listOf(set(100.0, 10)), newWeightLb = 95.0, newReps = 8))
    }

    @Test
    fun equalToBestIsNotAPrStrictlyGreaterRequired() {
        assertFalse(PrDetector.isPr(listOf(set(100.0, 8)), newWeightLb = 100.0, newReps = 8))
    }

    @Test
    fun onlySetsAtSameOrHigherRepsCompete() {
        // A heavy low-rep set (200x3) shouldn't block a PR at a higher rep count (150x5).
        assertTrue(PrDetector.isPr(listOf(set(200.0, 3)), newWeightLb = 150.0, newReps = 5))
    }

    @Test
    fun priorSetsWithoutNumericWeightAreIgnored() {
        val history = listOf(set(null, 10), set(80.0, 10))
        assertTrue(PrDetector.isPr(history, newWeightLb = 85.0, newReps = 10))
    }
}
