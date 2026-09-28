package com.forge.app.domain.health

import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.toDisplayWeight

/**
 * The MET → energy formula behind [ActiveCalorieEstimator]'s resistance-training estimate. The lb → kg
 * step goes through the app's one weight conversion ([toDisplayWeight]) so it can't drift from it.
 *
 * Standard Compendium model: `kcal = MET × bodyweightKg × hours`.
 */
object MetCalories {

    /** Kilocalories for a [met] activity sustained over [minutes] (fractional ok) at [bodyweightLb]. */
    fun kcal(met: Double, bodyweightLb: Double, minutes: Double): Double =
        met * toDisplayWeight(bodyweightLb, WeightUnit.KG) * (minutes / 60.0)
}
