package com.forge.app.ui.gym.train.components

import com.forge.app.domain.units.WeightUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * The UP NEXT pill printed the advisor's raw pound Double with no unit: a kg user saw
 * "+2.299999999999997 ↑" and a plate machine read "+15" pounds for one plate (audit 2026-09-26).
 */
class UpNextDeltaLabelTest {

    @Test
    fun `a kg user reads kilograms, never the raw pound float`() {
        assertEquals("+1.0 kg ↑", upNextDeltaLabel(2.299999999999997, isPlates = false, WeightUnit.KG, plateLb = 15.0))
    }

    @Test
    fun `pounds keep one decimal and drop it when whole`() {
        assertEquals("+2.3 lb ↑", upNextDeltaLabel(2.299999999999997, isPlates = false, WeightUnit.LB, plateLb = 15.0))
        assertEquals("−5 lb ↓", upNextDeltaLabel(-5.0, isPlates = false, WeightUnit.LB, plateLb = 15.0))
    }

    @Test
    fun `plate exercises read a plate count`() {
        assertEquals("+1 pl ↑", upNextDeltaLabel(15.0, isPlates = true, WeightUnit.LB, plateLb = 15.0))
        assertEquals("−0.5 pl ↓", upNextDeltaLabel(-7.5, isPlates = true, WeightUnit.KG, plateLb = 15.0))
    }

    @Test
    fun `rounding noise and no suggestion show no pill`() {
        assertNull(upNextDeltaLabel(0.3, isPlates = false, WeightUnit.LB, plateLb = 15.0))
        assertNull(upNextDeltaLabel(null, isPlates = false, WeightUnit.KG, plateLb = 15.0))
    }
}
