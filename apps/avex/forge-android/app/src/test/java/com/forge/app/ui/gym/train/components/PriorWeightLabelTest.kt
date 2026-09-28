package com.forge.app.ui.gym.train.components

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.WeightUnit
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The card's plan line and the collapsed row printed last time's stored weight text, which is always
 * pounds, so a kg user read "last 220.5 × 5" for the 100 kg they lifted.
 */
class PriorWeightLabelTest {

    private fun set(weightText: String, weightLb: Double?) =
        LoggedSet(loggedExerciseId = 1L, setIndex = 0, weightText = weightText, weightLb = weightLb, reps = 5, completedAt = 1L)

    @Test
    fun `a kg user reads kilograms, not the stored pounds`() {
        assertEquals("100", priorWeightLabel(set("220.5", 220.46226218), isPlates = false, WeightUnit.KG))
    }

    @Test
    fun `a pound user reads the same number as before`() {
        assertEquals("135", priorWeightLabel(set("135", 135.0), isPlates = false, WeightUnit.LB))
    }

    @Test
    fun `plate counts and weightless sets keep their text`() {
        assertEquals("3", priorWeightLabel(set("3", 45.0), isPlates = true, WeightUnit.KG))
        assertEquals("BW", priorWeightLabel(set("BW", null), isPlates = false, WeightUnit.KG))
    }
}
