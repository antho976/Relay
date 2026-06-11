package com.forge.app.domain.coach

import com.forge.app.data.db.entities.SuggestionOutcome
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Auto-coach Phase 2 — the step calibrator. RestAdvisor discipline: exactly neutral below
 * the sample gates, bounded output, deterministic.
 */
class SuggestionCalibratorTest {

    private fun outcome(
        exerciseId: String = "ua1",
        unit: String = "db",
        suggested: Double = 25.0,
        taken: Double = 25.0,
        reps: Int = 10,
        range: String = "8-12",
        at: Long = 0
    ) = SuggestionOutcome(
        exerciseId = exerciseId, unit = unit, suggestedLb = suggested,
        takenLb = taken, reps = reps, rangeText = range, loggedAt = at
    )

    /** n samples where the user went heavier than suggested and still hit the range floor. */
    private fun heavierAndSucceeding(n: Int, exerciseId: String = "ua1") =
        (0 until n).map { outcome(exerciseId = exerciseId, suggested = 25.0, taken = 30.0, reps = 10, at = it.toLong()) }

    /** n samples where the suggestion was taken as-is and the set failed the range floor. */
    private fun takenAndFailing(n: Int, exerciseId: String = "ua1") =
        (0 until n).map { outcome(exerciseId = exerciseId, suggested = 25.0, taken = 25.0, reps = 5, at = it.toLong()) }

    @Test
    fun coldStart_neutralEverywhere() {
        val c = SuggestionCalibrator.calibrate(emptyList())
        assertEquals(StepMode.NEUTRAL, c.modeFor("ua1", "db"))
    }

    @Test
    fun belowExerciseGate_staysNeutral() {
        val c = SuggestionCalibrator.calibrate(heavierAndSucceeding(SuggestionCalibrator.MIN_SAMPLES_EXERCISE - 1))
        assertEquals(StepMode.NEUTRAL, c.modeFor("ua1", "db"))
    }

    @Test
    fun consistentSuccessfulOverrides_earnFasterSteps() {
        val c = SuggestionCalibrator.calibrate(heavierAndSucceeding(SuggestionCalibrator.MIN_SAMPLES_EXERCISE))
        assertEquals(StepMode.FASTER, c.modeFor("ua1", "db"))
    }

    @Test
    fun heavierButFailing_doesNotEarnFasterSteps() {
        val failing = (0 until SuggestionCalibrator.MIN_SAMPLES_EXERCISE)
            .map { outcome(suggested = 25.0, taken = 30.0, reps = 4, at = it.toLong()) }
        assertEquals(StepMode.NEUTRAL, SuggestionCalibrator.calibrate(failing).modeFor("ua1", "db"))
    }

    @Test
    fun takenSuggestionsThatKeepFailing_earnConsolidate() {
        val c = SuggestionCalibrator.calibrate(takenAndFailing(SuggestionCalibrator.MIN_SAMPLES_EXERCISE))
        assertEquals(StepMode.CONSOLIDATE, c.modeFor("ua1", "db"))
    }

    @Test
    fun takenAndSucceeding_staysNeutral() {
        val fine = (0 until SuggestionCalibrator.MIN_SAMPLES_EXERCISE)
            .map { outcome(reps = 10, at = it.toLong()) }
        assertEquals(StepMode.NEUTRAL, SuggestionCalibrator.calibrate(fine).modeFor("ua1", "db"))
    }

    @Test
    fun unitBucketFallback_coversExercisesBelowTheirOwnGate() {
        // 16 db samples spread over 4 exercises (4 each — all below the per-exercise gate of 8),
        // every one a successful heavier override → the "db" bucket goes FASTER and a fifth,
        // never-sampled db exercise inherits it.
        val spread = (0 until SuggestionCalibrator.MIN_SAMPLES_UNIT).map { i ->
            outcome(exerciseId = "ex${i % 4}", suggested = 25.0, taken = 30.0, reps = 10, at = i.toLong())
        }
        val c = SuggestionCalibrator.calibrate(spread)
        assertEquals(StepMode.NEUTRAL, c.byExercise["ex0"] ?: StepMode.NEUTRAL)
        assertEquals(StepMode.FASTER, c.modeFor("never-sampled", "db"))
    }

    @Test
    fun exerciseLevelOutranksUnitBucket() {
        // The bucket says FASTER, but this exercise's own 8 samples say CONSOLIDATE.
        val bucket = (0 until SuggestionCalibrator.MIN_SAMPLES_UNIT).map { i ->
            outcome(exerciseId = "other${i % 4}", suggested = 25.0, taken = 30.0, reps = 10, at = i.toLong())
        }
        val own = takenAndFailing(SuggestionCalibrator.MIN_SAMPLES_EXERCISE, exerciseId = "ua1")
        val c = SuggestionCalibrator.calibrate(bucket + own)
        assertEquals(StepMode.CONSOLIDATE, c.modeFor("ua1", "db"))
    }

    @Test
    fun calibrateIsDeterministic() {
        val data = heavierAndSucceeding(10) + takenAndFailing(8, exerciseId = "ua2")
        assertEquals(SuggestionCalibrator.calibrate(data), SuggestionCalibrator.calibrate(data))
    }
}
