package com.forge.app.ui.onboarding

import com.forge.app.domain.units.WeightUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Tapping the other unit chip on the onboarding closing step re-expresses the typed bodyweight, so
 * the number keeps meaning the same weight (H-10). It used to leave the text alone and change only
 * the parser's unit: "170" under lb, tap kg, and the first bodyweight row was logged at 170 kg —
 * about 375 lb — feeding the trend, the strength standards and the coach a value off by 2.2×.
 */
class BodyweightUnitTransitionTest {

    @Test
    fun `lb to kg keeps the weight and stores the original pounds`() {
        val text = convertBodyweightInput("170", fromKg = false, toKg = true)
        assertEquals("77.1", text)
        // What finishing onboarding now stores: still ~170 lb, not 170 kg.
        assertEquals(170.0, parseSaneBodyweightLb(text, useKg = true)!!, 0.1)
    }

    @Test
    fun `kg to lb keeps the weight and stores the same pounds`() {
        val text = convertBodyweightInput("80", fromKg = true, toKg = false)
        assertEquals("176.4", text)
        // 80 kg = 176.4 lb — the same row whichever chip is lit.
        assertEquals(80 * 2.20462, parseSaneBodyweightLb(text, useKg = false)!!, 0.05)
        assertEquals(parseSaneBodyweightLb("80", useKg = true)!!, parseSaneBodyweightLb(text, useKg = false)!!, 0.05)
    }

    @Test
    fun `the WeightUnit overload is the same transition`() {
        assertEquals("77.1", convertBodyweightInput("170", WeightUnit.LB, WeightUnit.KG))
        assertEquals("176.4", convertBodyweightInput("80", WeightUnit.KG, WeightUnit.LB))
    }

    @Test
    fun `a typed suffix states the unit the user meant`() {
        // "80 kg" typed while lb was lit already read as 80 kg (parseToLb honours the suffix), so
        // switching to kg shows the bare number it meant.
        assertEquals("80", convertBodyweightInput("80 kg", fromKg = false, toKg = true))
    }

    @Test
    fun `blank stays blank and the same unit is a no-op`() {
        assertEquals("", convertBodyweightInput("", fromKg = false, toKg = true))
        assertEquals("   ", convertBodyweightInput("   ", fromKg = true, toKg = false))
        assertEquals("170", convertBodyweightInput("170", fromKg = false, toKg = false))
        assertEquals("80", convertBodyweightInput("80", fromKg = true, toKg = true))
    }

    @Test
    fun `unreadable text stays as typed and keeps its error`() {
        val text = convertBodyweightInput("abc", fromKg = false, toKg = true)
        assertEquals("abc", text)
        assertNull("still flagged, not silently dropped", parseSaneBodyweightLb(text, useKg = true))
    }

    @Test
    fun `an out-of-range number converts and stays out of range`() {
        // 5000 lb was an error under lb; 2268 kg is the same error under kg — never a valid value
        // conjured by the unit change.
        val text = convertBodyweightInput("5000", fromKg = false, toKg = true)
        assertEquals("2268.0", text)
        assertNull(parseSaneBodyweightLb(text, useKg = true))
    }
}
