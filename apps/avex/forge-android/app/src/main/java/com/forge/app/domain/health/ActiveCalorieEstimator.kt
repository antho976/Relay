package com.forge.app.domain.health

/**
 * Pure, testable estimate of the active calories burned in a resistance-training session, so the
 * write-to-Health-Connect path (HC-4) stays a thin wrapper around a deterministic core — exactly
 * like [BodyweightSync] guards the bodyweight bridge.
 *
 * The model is the standard MET formula: `kcal = MET × bodyweightKg × hours`. The MET is chosen
 * from the session's logged intensity (compendium values for weight training: light/moderate ≈ 3.5,
 * vigorous ≈ 6.0). It's deliberately an *estimate* — Forge has no heart-rate stream — so it's only
 * ever written to HC as a manual entry, and only when the user has opted in.
 *
 * Returns null when the inputs can't support an estimate (no logged bodyweight, or a zero-length
 * session): the caller then writes nothing rather than inventing a number.
 */
object ActiveCalorieEstimator {

    // Compendium-of-Physical-Activities MET values for resistance training, bucketed by the
    // session's intensity intent ("light" | "normal" | "hard"). Conservative on purpose.
    private const val MET_LIGHT = 3.5
    private const val MET_NORMAL = 5.0
    private const val MET_HARD = 6.0

    /** MET for a session's logged [intensity]; anything unrecognised falls back to the normal bucket. */
    fun metFor(intensity: String): Double = when (intensity) {
        "light" -> MET_LIGHT
        "hard" -> MET_HARD
        else -> MET_NORMAL
    }

    /**
     * Estimated active kilocalories for [activeMinutes] of training at [bodyweightLb] and the given
     * [intensity], or null when [activeMinutes] or [bodyweightLb] is non-positive (no estimate possible).
     * Minutes are a Double so the caller can pass fractional minutes (e.g. activeSeconds / 60.0) instead
     * of truncating a sub-minute remainder to zero. Shares [MetCalories] with the cardio estimator.
     */
    fun estimate(activeMinutes: Double, bodyweightLb: Double, intensity: String): Double? {
        if (activeMinutes <= 0.0 || bodyweightLb <= 0.0) return null
        return MetCalories.kcal(metFor(intensity), bodyweightLb, activeMinutes)
    }
}
