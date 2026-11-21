package com.forge.app.domain.health

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ActiveCalorieEstimatorTest {

    @Test fun `null when no bodyweight`() {
        assertNull(ActiveCalorieEstimator.estimate(activeMinutes = 60.0, bodyweightLb = 0.0, intensity = "normal"))
    }

    @Test fun `null when zero-length session`() {
        assertNull(ActiveCalorieEstimator.estimate(activeMinutes = 0.0, bodyweightLb = 180.0, intensity = "normal"))
    }

    @Test fun `normal-intensity hour matches MET formula`() {
        // kcal = 5.0 MET × (180 lb × 0.45359237) kg × 1 h ≈ 408.2
        val kcal = ActiveCalorieEstimator.estimate(60.0, 180.0, "normal")!!
        assertEquals(408.2, kcal, 0.5)
    }

    @Test fun `intensity scales the burn`() {
        val light = ActiveCalorieEstimator.estimate(60.0, 180.0, "light")!!
        val normal = ActiveCalorieEstimator.estimate(60.0, 180.0, "normal")!!
        val hard = ActiveCalorieEstimator.estimate(60.0, 180.0, "hard")!!
        assertTrue(light < normal)
        assertTrue(normal < hard)
    }

    @Test fun `unknown intensity falls back to normal`() {
        assertEquals(ActiveCalorieEstimator.metFor("normal"), ActiveCalorieEstimator.metFor("anything-else"), 0.0)
    }

    @Test fun `half-length session is half the calories`() {
        val full = ActiveCalorieEstimator.estimate(60.0, 200.0, "hard")!!
        val half = ActiveCalorieEstimator.estimate(30.0, 200.0, "hard")!!
        assertEquals(full / 2.0, half, 0.001)
    }

    @Test fun `fractional minutes are not truncated`() {
        // 90 active seconds = 1.5 min must not collapse to 1 min (the old Int-division bug).
        val oneAndHalf = ActiveCalorieEstimator.estimate(90.0 / 60.0, 180.0, "normal")!!
        val oneMin = ActiveCalorieEstimator.estimate(1.0, 180.0, "normal")!!
        assertEquals(oneMin * 1.5, oneAndHalf, 0.001)
    }
}
