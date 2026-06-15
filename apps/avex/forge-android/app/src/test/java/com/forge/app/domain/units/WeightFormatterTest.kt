package com.forge.app.domain.units

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import java.util.Locale

/**
 * Round-trip coverage for the centralized weight helpers (#41 / #23). The log + edit fields
 * hold a bare number in the display unit; [toStoredWeightText] / [parseToLb] convert to lb.
 * If these diverge, kg entries silently log with no weight — exactly the bug #23 fixed.
 */
class WeightFormatterTest {

    private val kgPerLb = 0.45359237

    @Before
    fun pinLocale() {
        // formatWeight/weightInputValue use String.format; pin so the decimal separator is '.'
        // (matching what parseToLb expects) regardless of the CI/runner default locale.
        Locale.setDefault(Locale.US)
    }

    @Test
    fun formatLbWholeNumberHasNoDecimal() {
        assertEquals("100 lb", formatWeight(100.0, useKg = false))
    }

    @Test
    fun formatKgConvertsAndLabels() {
        assertEquals("45.4 kg", formatWeight(100.0, useKg = true)) // 100 lb = 45.359 kg
    }

    @Test
    fun inputValueRoundTripsInLb() {
        val text = weightInputValue(135.0, useKg = false)
        assertEquals(135.0, parseToLb(text, useKg = false)!!, 0.001)
    }

    @Test
    fun inputValueRoundTripsInKgWithinDisplayPrecision() {
        val text = weightInputValue(100.0, useKg = true) // "45.4"
        assertEquals(100.0, parseToLb(text, useKg = true)!!, 0.5) // display rounds to 0.1 kg
    }

    @Test
    fun parseToLbToleratesUnitSuffix() {
        assertEquals(20.0 / kgPerLb, parseToLb("20 kg", useKg = true)!!, 0.001)
        assertEquals(20.0 / kgPerLb, parseToLb("20kg", useKg = true)!!, 0.001)
        assertEquals(20.0, parseToLb("20 lb", useKg = false)!!, 0.001)
    }

    @Test
    fun parseToLbRejectsNonNumeric() {
        assertNull(parseToLb("BW", useKg = false))
        assertNull(parseToLb("two plates", useKg = false))
    }

    @Test
    fun toStoredWeightTextConvertsKgEntryToLb() {
        assertEquals("44.1", toStoredWeightText("20", useKg = true)) // 20 kg = 44.09 lb
    }

    @Test
    fun toStoredWeightTextPassesLbThrough() {
        assertEquals("135", toStoredWeightText("135", useKg = false))
    }

    @Test
    fun toStoredWeightTextPassesNonNumericThrough() {
        // "BW" / "2 plates" must survive untouched for WeightParser to interpret.
        assertEquals("BW", toStoredWeightText("BW", useKg = true))
    }

    @Test
    fun formatWeightDeltaLabelsInDisplayUnit() {
        assertEquals("5 lb", formatWeightDelta(5.0, useKg = false))
    }

    @Test
    fun formatVolumeLbWholeNumber() {
        assertEquals("850 lb", formatVolume(850.0, useKg = false))
    }

    @Test
    fun formatVolumeLbAbbreviatesPastThousand() {
        assertEquals("1.2k lb", formatVolume(1234.0, useKg = false))
    }

    @Test
    fun formatVolumeKgConvertsBelowThousand() {
        // 500 lb = 226.8 kg (the unit setting must convert volume, not just weight — #2)
        assertEquals("226 kg", formatVolume(500.0, useKg = true))
    }

    @Test
    fun formatVolumeKgAbbreviatesOnConvertedValue() {
        // 5000 lb = 2268 kg — the 1k abbreviation applies AFTER conversion, so it reads "2.3k kg".
        assertEquals("2.3k kg", formatVolume(5000.0, useKg = true))
    }
}
