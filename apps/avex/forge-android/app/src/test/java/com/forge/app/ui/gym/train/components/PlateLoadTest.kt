package com.forge.app.ui.gym.train.components

import org.junit.Assert.assertEquals
import org.junit.Test

class PlateLoadTest {
    private val lb = listOf(45.0, 35.0, 25.0, 10.0, 5.0, 2.5)
    private val kg = listOf(25.0, 20.0, 15.0, 10.0, 5.0, 2.5, 1.25)

    @Test fun `an exact target loads exactly`() {
        val load = plateLoadUnder(225.0, 45.0, lb)
        assertEquals(225.0, load.total, 1e-9)
        assertEquals(listOf(45.0 to 2), load.plates)
    }

    @Test fun `a target the plates can't make loads the closest under it`() {
        // 137 lb on a 45 lb bar is 46 per side: 45 + nothing smaller than 2.5 fits the last pound.
        val load = plateLoadUnder(137.0, 45.0, lb)
        assertEquals(135.0, load.total, 1e-9)
        assertEquals(listOf(45.0 to 1), load.plates)
    }

    @Test fun `kilo plates and a kilo bar`() {
        val load = plateLoadUnder(102.5, 20.0, kg)
        assertEquals(102.5, load.total, 1e-9)
        assertEquals(listOf(25.0 to 1, 15.0 to 1, 1.25 to 1), load.plates)
    }

    @Test fun `float noise from a unit conversion doesn't drop a plate`() {
        assertEquals(100.0, plateLoadUnder(99.99999999, 20.0, kg).total, 1e-9)
    }
}
