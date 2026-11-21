package com.forge.app.ui.onboarding

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Onboarding bodyweight sanity guard — a fat-finger must not poison the relative-strength ratio. */
class BodyweightValidationTest {

    @Test
    fun acceptsRealisticWeights() {
        assertEquals(180.0, parseSaneBodyweightLb("180", useKg = false)!!, 0.001)
        // 80 kg ≈ 176.4 lb
        assertEquals(80 * 2.20462, parseSaneBodyweightLb("80", useKg = true)!!, 0.001)
    }

    @Test
    fun rejectsTheFatFingerOneKg() {
        // "1" kg ≈ 2.2 lb — the 50× ratio bug. Must be rejected, not logged.
        assertNull(parseSaneBodyweightLb("1", useKg = true))
        assertNull(parseSaneBodyweightLb("1", useKg = false))
    }

    @Test
    fun rejectsAbsurdlyHighAndUnparseable() {
        assertNull(parseSaneBodyweightLb("5000", useKg = false))
        assertNull(parseSaneBodyweightLb("", useKg = false))
        assertNull(parseSaneBodyweightLb("abc", useKg = false))
    }
}
