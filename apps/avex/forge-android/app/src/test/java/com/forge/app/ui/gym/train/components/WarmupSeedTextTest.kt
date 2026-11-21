package com.forge.app.ui.gym.train.components

import com.forge.app.domain.units.WeightUnit
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The warm-up suggester's field is a plate COUNT on a plate machine, but every load reaches it in
 * stored pounds. Passing the pounds straight through showed "60 Working plates" for a four-plate
 * set and built the ramp from sixty plates.
 */
class WarmupSeedTextTest {

    @Test
    fun `a plate machine seeds a plate count, not the stored pounds`() {
        assertEquals("4", warmupSeedText(60.0, isPlates = true, plateLb = 15.0, weightUnit = WeightUnit.LB))
        assertEquals("4.5", warmupSeedText(67.5, isPlates = true, plateLb = 15.0, weightUnit = WeightUnit.LB))
        // The display unit never touches a plate count.
        assertEquals("3", warmupSeedText(60.0, isPlates = true, plateLb = 20.0, weightUnit = WeightUnit.KG))
    }

    @Test
    fun `every other exercise seeds the display-unit weight`() {
        assertEquals("60", warmupSeedText(60.0, isPlates = false, plateLb = 15.0, weightUnit = WeightUnit.LB))
        assertEquals("27.2", warmupSeedText(60.0, isPlates = false, plateLb = 15.0, weightUnit = WeightUnit.KG))
    }

    @Test
    fun `an unusable plate weight falls back to pounds instead of infinity`() {
        assertEquals("60", warmupSeedText(60.0, isPlates = true, plateLb = 0.0, weightUnit = WeightUnit.LB))
    }
}
