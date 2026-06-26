package com.forge.app.domain.units

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import java.util.Locale

/**
 * Round-trip coverage for the centralized distance helpers (km↔display). Distance is stored in km
 * everywhere; the cardio log field holds a bare number in the display unit and [parseToKm] converts
 * back. If [distanceInputValue] / [parseToKm] diverge, a miles entry would silently log the wrong km.
 */
class DistanceFormatterTest {

    private val kmPerMile = 1.609344

    @Before
    fun pinLocale() {
        // formatDistance/distanceInputValue use String.format; pin so the decimal separator is '.'
        // (matching what parseToKm expects) regardless of the runner's default locale.
        Locale.setDefault(Locale.US)
    }

    @Test
    fun formatKmKeepsOneDecimal() {
        assertEquals("5.0 km", formatDistance(5.0, useMiles = false))
    }

    @Test
    fun formatMilesConvertsAndLabels() {
        // 5 km = 3.107 mi → "3.1 mi"
        assertEquals("3.1 mi", formatDistance(5.0, useMiles = true))
    }

    @Test
    fun inputValueRoundTripsInKm() {
        val text = distanceInputValue(8.0, useMiles = false)
        assertEquals(8.0, parseToKm(text, useMiles = false)!!, 0.001)
    }

    @Test
    fun inputValueRoundTripsInMilesWithinDisplayPrecision() {
        val text = distanceInputValue(5.0, useMiles = true) // "3.1"
        assertEquals(5.0, parseToKm(text, useMiles = true)!!, 0.05) // display rounds to 0.1 mi
    }

    @Test
    fun wholeMileValueDropsTrailingZero() {
        // 1 mile = 1.609344 km; seeding the field should read a clean "1", not "1.0".
        assertEquals("1", distanceInputValue(kmPerMile, useMiles = true))
    }

    @Test
    fun parseToKmToleratesUnitSuffix() {
        assertEquals(kmPerMile, parseToKm("1 mi", useMiles = true)!!, 0.001)
        assertEquals(kmPerMile, parseToKm("1mi", useMiles = true)!!, 0.001)
        assertEquals(5.0, parseToKm("5 km", useMiles = false)!!, 0.001)
    }

    @Test
    fun parseToKmRejectsBlankOrNonNumeric() {
        assertNull(parseToKm("", useMiles = false))
        assertNull(parseToKm("abc", useMiles = true))
    }
}
