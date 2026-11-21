package com.forge.app.ui.onboarding

import com.forge.app.domain.units.fromDisplayWeight
import org.junit.Assert.assertEquals
import org.junit.Test

class SnapPlateToUnitTest {
    @Test
    fun aPoundPlateBecomesTheNearestKiloPlate() {
        // 15 lb is 6.8 kg: the nearest kilo plate is 5 kg.
        assertEquals(fromDisplayWeight(5.0, true), snapPlateToUnit(15.0, toKg = true), 1e-9)
        // 45 lb is 20.4 kg.
        assertEquals(fromDisplayWeight(20.0, true), snapPlateToUnit(45.0, toKg = true), 1e-9)
    }

    @Test
    fun aKiloPlateBecomesTheNearestPoundPlate() {
        // 10 kg is 22.05 lb: the nearest pound plate is 20.
        assertEquals(20.0, snapPlateToUnit(fromDisplayWeight(10.0, true), toKg = false), 1e-9)
    }
}
