package com.forge.shared.weight

import org.junit.Assert.assertEquals
import org.junit.Test

class WeightStepsTest {

    @Test
    fun `steps match the phone stepper table`() {
        assertEquals(5.0, WeightSteps.weightStep(ProtocolWeightUnit.LB, isPlates = false), 0.0)
        assertEquals(2.5, WeightSteps.weightStep(ProtocolWeightUnit.KG, isPlates = false), 0.0)
        assertEquals(0.5, WeightSteps.weightStep(ProtocolWeightUnit.ST, isPlates = false), 0.0)
    }

    @Test
    fun `plate exercises step by half a plate in every unit`() {
        ProtocolWeightUnit.entries.forEach { unit ->
            assertEquals(0.5, WeightSteps.weightStep(unit, isPlates = true), 0.0)
        }
    }
}
