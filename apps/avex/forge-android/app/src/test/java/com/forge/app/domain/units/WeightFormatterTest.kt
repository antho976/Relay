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

    // ─── Stones (GYMAP-72) — stone + pounds compound display, decimal aggregates ──────────

    @Test
    fun formatStonesAsCompound() {
        assertEquals("9 st 9 lb", formatWeight(135.0, WeightUnit.ST))  // 135 = 9*14 + 9
        assertEquals("12 st", formatWeight(168.0, WeightUnit.ST))      // exact stone, no lb remainder
        assertEquals("8 lb", formatWeight(8.0, WeightUnit.ST))         // under a stone → lb only
    }

    @Test
    fun formatVolumeStonesStaysDecimal() {
        assertEquals("100 st", formatVolume(1400.0, WeightUnit.ST))    // 1400 / 14 = 100
        assertEquals("1.4k st", formatVolume(20000.0, WeightUnit.ST))  // 1428.6 st → abbreviated
    }

    @Test
    fun parseStonesCompoundAndBare() {
        assertEquals(172.0, parseToLb("12 st 4 lb", WeightUnit.ST)!!, 0.001) // 12*14 + 4
        assertEquals(172.0, parseToLb("12 st 4", WeightUnit.ST)!!, 0.001)
        assertEquals(168.0, parseToLb("12 st", WeightUnit.ST)!!, 0.001)
        assertEquals(134.4, parseToLb("9.6", WeightUnit.ST)!!, 0.001)        // bare decimal stones
        assertNull(parseToLb("BW", WeightUnit.ST))
    }

    @Test
    fun stonesInputValueRoundTripsWithinDisplayPrecision() {
        assertEquals("12", weightInputValue(168.0, WeightUnit.ST))           // exact → no decimal
        val text = weightInputValue(135.0, WeightUnit.ST)                    // "9.6"
        assertEquals(135.0, parseToLb(text, WeightUnit.ST)!!, 1.5)           // display rounds to 0.1 st
    }

    @Test
    fun toStoredWeightTextStonesConvertsToLb() {
        assertEquals("172", toStoredWeightText("12 st 4", WeightUnit.ST))
        assertEquals("BW", toStoredWeightText("BW", WeightUnit.ST))
    }

    @Test
    fun formatWeightDeltaStones() {
        assertEquals("1 st 6 lb", formatWeightDelta(20.0, WeightUnit.ST))    // 20 = 14 + 6
        assertEquals("6 lb", formatWeightDelta(6.0, WeightUnit.ST))          // under a stone, no "+"
        assertEquals("-8 lb", formatWeightDelta(-8.0, WeightUnit.ST))        // loss keeps the sign
    }
}
