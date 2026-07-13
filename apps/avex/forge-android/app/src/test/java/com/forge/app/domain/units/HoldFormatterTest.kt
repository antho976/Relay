package com.forge.app.domain.units

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import java.util.Locale

/**
 * Coverage for the timed-hold duration helpers (GYMAP-51). Holds are stored as whole seconds; the
 * logger field shows/accepts a `m:ss` stopwatch reading and [parseHold] converts back. If format and
 * parse diverge, a hold entry would silently log the wrong duration.
 */
class HoldFormatterTest {

    @Before
    fun pinLocale() {
        // formatHold uses String.format; pin so the ':' and digits never localise.
        Locale.setDefault(Locale.US)
    }

    @Test
    fun formatHold_rendersStopwatchReading() {
        assertEquals("0:00", formatHold(0))
        assertEquals("0:45", formatHold(45))
        assertEquals("1:30", formatHold(90))
        assertEquals("10:05", formatHold(605))
        assertEquals("0:00", formatHold(-5)) // negatives clamp
    }

    @Test
    fun formatHoldLabel_usesSecondsSuffixUnderAMinute() {
        assertEquals("45s", formatHoldLabel(45))
        assertEquals("0s", formatHoldLabel(0))
        assertEquals("1:30", formatHoldLabel(90))
        assertEquals("2:00", formatHoldLabel(120))
    }

    @Test
    fun parseHold_bareNumberIsSeconds() {
        assertEquals(45, parseHold("45"))
        assertEquals(90, parseHold("90"))
        assertEquals(0, parseHold("0"))
    }

    @Test
    fun parseHold_mmSs() {
        assertEquals(90, parseHold("1:30"))
        assertEquals(605, parseHold("10:05"))
        assertEquals(30, parseHold(":30"))     // empty minutes → 0
        assertEquals(150, parseHold("2:30"))
    }

    @Test
    fun parseHold_toleratesSecondsSuffix() {
        assertEquals(45, parseHold("45s"))
        assertEquals(90, parseHold("1:30s"))
    }

    @Test
    fun parseHold_secondsFieldTakenModSixty() {
        // "1:90" is malformed; the seconds part is taken mod 60 (90 % 60 = 30) so it reads 1:30 = 90s
        // rather than silently inflating to 2:30. A well-formed mm:ss field never produces this.
        assertEquals(90, parseHold("1:90"))
    }

    @Test
    fun parseHold_roundTripsFormat() {
        listOf(1, 30, 45, 59, 60, 90, 125, 600, 3599).forEach { s ->
            assertEquals("round-trip $s", s, parseHold(formatHold(s)))
        }
    }

    @Test
    fun parseHold_clampsToCeiling() {
        assertEquals(MAX_HOLD_SECONDS, parseHold("99999"))
        assertEquals(MAX_HOLD_SECONDS, parseHold("120:00"))
    }

    @Test
    fun parseHold_rejectsGarbage() {
        assertNull(parseHold(""))
        assertNull(parseHold("   "))
        assertNull(parseHold("abc"))
        assertNull(parseHold("1:2:3"))
        assertNull(parseHold("1:x"))
    }
}
