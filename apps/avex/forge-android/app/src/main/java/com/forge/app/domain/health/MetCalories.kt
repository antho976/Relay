package com.forge.app.domain.health

/**
 * The one MET → energy formula shared by both calorie estimators — resistance training in
 * [ActiveCalorieEstimator] and cardio in [com.forge.app.domain.cardio.CardioCalorieEstimator] — so a
 * correction to the conversion (or the kg constant) can never make the two drift apart.
 *
 * Standard Compendium model: `kcal = MET × bodyweightKg × hours`.
 */
object MetCalories {

    const val LB_TO_KG = 0.45359237

    /** Kilocalories for a [met] activity sustained over [minutes] (fractional ok) at [bodyweightLb]. */
    fun kcal(met: Double, bodyweightLb: Double, minutes: Double): Double =
        met * (bodyweightLb * LB_TO_KG) * (minutes / 60.0)
}
