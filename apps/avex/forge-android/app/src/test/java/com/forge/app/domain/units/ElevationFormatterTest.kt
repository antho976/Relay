package com.forge.app.domain.units

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import java.util.Locale

/**
 * Coverage for the elevation-gain helpers (GYMAP-38). Gain is stored canonically in metres and shown
 * in metres or feet off the DISTANCE unit toggle; if format and parse diverge, an entry would log the
 * wrong climb. Metric round-trips exactly (whole metres); imperial carries the ft↔m rounding.
 */
class ElevationFormatterTest {

    @Before
    fun pinLocale() {
        Locale.setDefault(Locale.US)
    }

    @Test
    fun unitLabel_followsMilesToggle() {
        assertEquals("m", elevationUnitLabel(false))
        assertEquals("ft", elevationUnitLabel(true))
    }

    @Test
    fun formatElevation_metricIsWholeMetres() {
        assertEquals("120 m", formatElevation(120.0, false))
        assertEquals("0 m", formatElevation(0.0, false))
        // 120.4 rounds down, 120.6 rounds up — no stray decimals ever reach the label.
        assertEquals("120 m", formatElevation(120.4, false))
        assertEquals("121 m", formatElevation(120.6, false))
    }

    @Test
    fun formatElevation_imperialConvertsAndRounds() {
        // 120 m = 393.7 ft → 394 ft.
        assertEquals("394 ft", formatElevation(120.0, true))
        assertEquals("0 ft", formatElevation(0.0, true))
    }

    @Test
    fun inputValue_seedsBareWholeNumber() {
        assertEquals("120", elevationInputValue(120.0, false))
        assertEquals("394", elevationInputValue(120.0, true))
    }

    @Test
    fun parseToMeters_metricIsIdentity() {
        assertEquals(120.0, parseToMeters("120", false)!!, 0.0001)
        assertEquals(0.0, parseToMeters("0", false)!!, 0.0001)
    }

    @Test
    fun parseToMeters_imperialConvertsBackToMetres() {
        // 394 ft ≈ 120.1 m.
        assertEquals(120.09, parseToMeters("394", true)!!, 0.1)
    }

    @Test
    fun parseToMeters_toleratesUnitSuffix() {
        assertEquals(120.0, parseToMeters("120 m", false)!!, 0.0001)
        assertEquals(120.0, parseToMeters("120m", false)!!, 0.0001)
        assertEquals(120.09, parseToMeters("394ft", true)!!, 0.1)
    }

    @Test
    fun parseToMeters_metricRoundTripsExact() {
        listOf(0.0, 15.0, 120.0, 999.0, 3500.0).forEach { m ->
            val back = parseToMeters(elevationInputValue(m, false), false)
            assertEquals("round-trip $m", m, back!!, 0.0001)
        }
    }

    @Test
    fun parseToMeters_rejectsGarbage() {
        assertNull(parseToMeters("", false))
        assertNull(parseToMeters("   ", false))
        assertNull(parseToMeters("abc", false))
    }
}
