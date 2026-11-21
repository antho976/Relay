package com.forge.app.ui.cardio.components

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * GYMAP-41 — the cardio duration field accepts either plain minutes ("90") or an H:MM clock value
 * ("1:30" -> 90). The store is whole minutes, so there is no seconds component.
 */
class CardioDurationInputTest {

    @Test
    fun plainNumberStaysMinutes() {
        assertEquals(30, parseDurationMin("30"))
        assertEquals(90, parseDurationMin("90"))
        assertEquals(5, parseDurationMin("5"))
    }

    @Test
    fun clockValueIsHoursTimesSixtyPlusMinutes() {
        assertEquals(90, parseDurationMin("1:30"))
        assertEquals(125, parseDurationMin("2:05"))
        assertEquals(45, parseDurationMin("0:45"))
        assertEquals(60, parseDurationMin("1:00"))
    }

    @Test
    fun emptyOrIncompleteIsZero() {
        assertEquals(0, parseDurationMin(""))
        assertEquals(0, parseDurationMin(":"))
        assertEquals(0, parseDurationMin("0:00"))
        // A leading colon means zero hours; the minutes still count.
        assertEquals(15, parseDurationMin(":15"))
    }

    @Test
    fun sanitizeKeepsDigitsAndOneColon() {
        assertEquals("130", sanitizeDuration("1a3b0"))
        assertEquals("1:30", sanitizeDuration("1:30"))
        // Only the first colon survives; later ones are dropped.
        assertEquals("1:30", sanitizeDuration("1:3:0"))
        // Capped at HH:MM width.
        assertEquals("12:34", sanitizeDuration("12:3456"))
    }

    @Test
    fun capDurationTextHoldsTheFieldToOneDay() {
        assertEquals("1440", capDurationText("99999"))
        assertEquals("1440", capDurationText("99:99"))
        assertEquals("1440", capDurationText("1500"))
        // Plausible values pass through untouched, including a whole day.
        assertEquals("90", capDurationText("90"))
        assertEquals("1:30", capDurationText("1:30"))
        assertEquals("1440", capDurationText("1440"))
        assertEquals("", capDurationText(""))
    }

    @Test
    fun capDistanceTextHoldsTheFieldToTheSingleEntryBound() {
        // Over the 1000 km bound, the field shows the bound itself — in the display unit.
        assertEquals("1000", capDistanceText("1500", useMiles = false))
        assertEquals("1000", capDistanceText("999999", useMiles = false))
        assertEquals("621.3", capDistanceText("999", useMiles = true))
        // The capped miles text never parses past the bound itself.
        assertEquals("621.3", capDistanceText("621.3", useMiles = true))
        // Plausible values and partial input pass through untouched.
        assertEquals("1000", capDistanceText("1000", useMiles = false))
        assertEquals("5.", capDistanceText("5.", useMiles = false))
        assertEquals("", capDistanceText("", useMiles = true))
    }
}
