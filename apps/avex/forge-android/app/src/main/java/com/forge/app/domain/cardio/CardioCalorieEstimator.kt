package com.forge.app.domain.cardio

import com.forge.app.domain.health.MetCalories
import kotlin.math.roundToInt

/**
 * Pure, testable estimate of the calories burned in a cardio session — the same MET model the
 * gym side uses ([com.forge.app.domain.health.ActiveCalorieEstimator]), but with a per-activity
 * base MET instead of an intensity bucket: `kcal = MET × bodyweightKg × hours`.
 *
 * It's deliberately an estimate (Avex logs cardio by hand, with no heart-rate stream), so it only
 * ever shows as an "≈" readout. Returns null when no honest estimate is possible — a rest day, a
 * zero-length entry, or no logged bodyweight to scale by — rather than inventing a number.
 */
object CardioCalorieEstimator {

    /** Compendium-of-Physical-Activities MET for each activity at a moderate pace. Rest burns nothing. */
    private fun baseMet(type: CardioType): Double = when (type) {
        CardioType.RUN -> 9.0
        CardioType.WALK -> 3.5
        CardioType.TREADMILL -> 6.0
        CardioType.CYCLE -> 7.5
        CardioType.SWIM -> 7.0
        CardioType.ROW -> 7.0
        CardioType.HIKE -> 6.0
        CardioType.ELLIPTICAL -> 5.0
        CardioType.HIIT -> 8.0
        CardioType.YOGA -> 2.5
        CardioType.OTHER -> 5.0
        CardioType.REST -> 0.0
    }

    /** Self-rated effort nudges the base MET up or down; an unrated entry counts as moderate. */
    private fun effortMultiplier(effort: CardioEffort?): Double = when (effort) {
        CardioEffort.EASY -> 0.85
        CardioEffort.HARD -> 1.2
        CardioEffort.MODERATE, null -> 1.0
    }

    /**
     * Estimated kilocalories for the session, or null when no estimate is possible (rest type,
     * non-positive duration, or no/zero bodyweight to scale by).
     */
    fun estimate(type: CardioType, durationMin: Int, effort: CardioEffort?, bodyweightLb: Double?): Int? {
        if (type.isRest || durationMin <= 0) return null
        val weightLb = bodyweightLb ?: return null
        if (weightLb <= 0.0) return null
        return MetCalories.kcal(baseMet(type) * effortMultiplier(effort), weightLb, durationMin.toDouble())
            .roundToInt()
    }
}
