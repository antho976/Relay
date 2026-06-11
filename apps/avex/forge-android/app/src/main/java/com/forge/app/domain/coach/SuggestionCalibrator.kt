package com.forge.app.domain.coach

import com.forge.app.data.db.entities.SuggestionOutcome
import com.forge.app.domain.adapt.RepRange
import kotlin.math.abs

/** How the weight chip should behave for a lift, learned from suggestion outcomes. */
enum class StepMode {
    /** Below the sample gates or no clear pattern — canonical behavior, untouched. */
    NEUTRAL,
    /** User consistently beats suggestions and succeeds — bigger steps fit them. */
    FASTER,
    /** Taken suggestions keep failing — hold the weight and own it before the next jump. */
    CONSOLIDATE
}

/**
 * Per-exercise step calibration with a unit-bucket fallback (hardening decision 7's
 * hierarchy): an exercise speaks for itself once it has [SuggestionCalibrator.MIN_SAMPLES_EXERCISE]
 * samples; below that its unit bucket ("db"/"plates") speaks once IT has
 * [SuggestionCalibrator.MIN_SAMPLES_UNIT]; below both, exactly neutral — never a guess.
 */
data class StepCalibration(
    val byExercise: Map<String, StepMode> = emptyMap(),
    val byUnit: Map<String, StepMode> = emptyMap()
) {
    fun modeFor(exerciseId: String, unitCode: String): StepMode =
        byExercise[exerciseId] ?: byUnit[unitCode] ?: StepMode.NEUTRAL

    companion object {
        val NEUTRAL = StepCalibration()
    }
}

/**
 * Auto-coach Phase 2 — the coach learns whether its weight advice worked. Pure function of
 * [SuggestionOutcome] rows, mirroring RestAdvisor's discipline: minimum qualified samples per
 * level, exactly-neutral below the gate, bounded output (an enum, not an open multiplier).
 *
 * Signals per group:
 *  - FASTER: at least half the samples took MORE than suggested, and those heavier picks
 *    succeeded (hit the rep-range floor) at least [FAST_SUCCESS_SHARE] of the time. The user's
 *    behavior is the evidence the step is too small.
 *  - CONSOLIDATE: among samples that took the suggestion as-is, at least [CONSOLIDATE_FAIL_SHARE]
 *    failed the range floor — jumps aren't sticking, hold before the next one.
 */
object SuggestionCalibrator {

    const val MIN_SAMPLES_EXERCISE = 8
    const val MIN_SAMPLES_UNIT = 16
    private const val OVERRIDE_TOLERANCE_LB = 0.1
    private const val FAST_OVERRIDE_SHARE = 0.5
    private const val FAST_SUCCESS_SHARE = 0.6
    private const val CONSOLIDATE_FAIL_SHARE = 0.5

    fun calibrate(outcomes: List<SuggestionOutcome>): StepCalibration {
        if (outcomes.isEmpty()) return StepCalibration.NEUTRAL
        val byExercise = outcomes.groupBy { it.exerciseId }
            .filterValues { it.size >= MIN_SAMPLES_EXERCISE }
            .mapValues { (_, group) -> modeFor(group) }
            .filterValues { it != StepMode.NEUTRAL }
        val byUnit = outcomes.groupBy { it.unit }
            .filterValues { it.size >= MIN_SAMPLES_UNIT }
            .mapValues { (_, group) -> modeFor(group) }
            .filterValues { it != StepMode.NEUTRAL }
        return StepCalibration(byExercise = byExercise, byUnit = byUnit)
    }

    private fun modeFor(group: List<SuggestionOutcome>): StepMode {
        val heavier = group.filter { it.takenLb > it.suggestedLb + OVERRIDE_TOLERANCE_LB }
        val asIs = group.filter { abs(it.takenLb - it.suggestedLb) <= OVERRIDE_TOLERANCE_LB }

        if (heavier.size.toDouble() / group.size >= FAST_OVERRIDE_SHARE &&
            heavier.count { succeeded(it) }.toDouble() / heavier.size >= FAST_SUCCESS_SHARE
        ) return StepMode.FASTER

        if (asIs.size >= group.size / 2 &&
            asIs.count { !succeeded(it) }.toDouble() / asIs.size >= CONSOLIDATE_FAIL_SHARE
        ) return StepMode.CONSOLIDATE

        return StepMode.NEUTRAL
    }

    /** Success = the first set reached the rep-range floor. Unparseable range = benefit of the doubt. */
    private fun succeeded(o: SuggestionOutcome): Boolean {
        val range = RepRange.parse(o.rangeText) ?: return true
        return o.reps >= range.min
    }
}
