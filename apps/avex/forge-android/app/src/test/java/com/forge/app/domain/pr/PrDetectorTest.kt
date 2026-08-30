package com.forge.app.domain.pr

import com.forge.app.data.db.entities.LoggedSet
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PrDetectorTest {

    private fun set(weightLb: Double?, reps: Int, assisted: Boolean = false) = LoggedSet(
        loggedExerciseId = 1L,
        setIndex = 0,
        weightText = weightLb?.toString() ?: "BW",
        weightLb = weightLb,
        reps = reps,
        completedAt = 0L,
        isAssisted = assisted
    )

    /**
     * Assisted history must not raise the bar a real PR has to clear.
     *
     * Found by mutation testing: deleting `!it.isAssisted` from the filter left the whole suite
     * green. The DAO-level equivalent was covered, so the app agreed with itself about the stored
     * maximum while THIS check — the one that decides whether the gold PR moment fires — did not
     * care. The two must exclude the same sets or the app contradicts itself: a lift the PR engine
     * refuses to recognise cannot also be the thing blocking recognition.
     */
    @Test
    fun assistedHistoryDoesNotBlockARealPr() {
        val assistedHeavier = listOf(set(200.0, 5, assisted = true))
        assertTrue(
            "an unassisted 150 is still a PR when the only heavier set was assisted",
            PrDetector.isPr(assistedHeavier, newWeightLb = 150.0, newReps = 5)
        )
    }

    @Test
    fun anAssistedSetIsNotTheThingBeingBeaten() {
        // Mixed history: the real best is 140 unassisted; the 200 was machine-assisted.
        val history = listOf(set(140.0, 5), set(200.0, 5, assisted = true))
        assertTrue(PrDetector.isPr(history, newWeightLb = 145.0, newReps = 5))
        assertFalse(
            "and the unassisted best is still enforced",
            PrDetector.isPr(history, newWeightLb = 139.0, newReps = 5)
        )
    }

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

    // ── Timed holds (finding 14) ──────────────────────────────────────────────

    private fun hold(weightLb: Double?, seconds: Int) = LoggedSet(
        loggedExerciseId = 1L, setIndex = 0, weightText = weightLb?.toString() ?: "BW",
        weightLb = weightLb, reps = seconds, completedAt = 0L, durationSeconds = seconds
    )

    @Test
    fun aTimedHoldIsNeverItselfAPr() {
        // Its `reps` column is a hold length. Nothing in a normal history reaches ninety "reps", so
        // the comparison set came back empty and every weighted plank flagged gold.
        assertFalse(
            "a 90-second weighted plank is not a rep-range record",
            PrDetector.isPr(emptyList(), newWeightLb = 45.0, newReps = 90, newDurationSeconds = 90)
        )
    }

    @Test
    fun timedHistoryDoesNotBlockARealPr() {
        // The other direction, and the one that lasts: a 90-second hold sat in history as a 90-rep
        // set at 45 lb, so a genuine 20-rep set at 40 lb was measured against it and refused.
        val history = listOf(hold(45.0, 90))
        assertTrue(
            "a real 20-rep set is a PR when the only 'heavier' entry is a hold",
            PrDetector.isPr(history, newWeightLb = 40.0, newReps = 20)
        )
    }

    @Test
    fun aRepSetOnATimedExerciseStillCompetesNormally() {
        // Only the duration decides. A set with no duration is a rep set even on a movement that is
        // usually held, and it is judged against the other rep sets exactly as before.
        val history = listOf(set(50.0, 10))
        assertFalse(PrDetector.isPr(history, newWeightLb = 45.0, newReps = 10))
        assertTrue(PrDetector.isPr(history, newWeightLb = 55.0, newReps = 10))
    }
}
