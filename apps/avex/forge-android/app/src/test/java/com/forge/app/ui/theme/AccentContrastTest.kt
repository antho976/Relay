package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * M-37: a label on an accent-filled control must clear WCAG AA, for every accent the picker accepts.
 *
 * The choice used to be a luminance threshold, which picks a side without knowing whether that side
 * reads. The theme's pair is a warm near-black and a warm near-white rather than the extremes, so
 * a band of accepted accents failed AA against both — the audit's `#777777` measures about 4.27:1
 * on the dark one and 3.9:1 on the light one.
 */
class AccentContrastTest {

    private val dark = Color(0xFF110F0C)   // the Pearl background
    private val light = PearlOnBg

    private fun on(accentHex: Long) = accentForeground(Color(accentHex), dark, light)
    private fun ratio(accentHex: Long) = contrastRatio(on(accentHex), Color(accentHex))

    @Test
    fun theAuditsCounterexampleNowClearsAA() {
        assertTrue("#777777 was 4.27:1 at best", ratio(0xFF777777) >= AA_CONTRAST)
    }

    @Test
    fun everyAccentAcrossTheAcceptedRangeClearsAA() {
        // Sweep the greys, which cover the luminance axis the choice actually turns on, plus the
        // band the finding names. A single failure here is a control whose label cannot be read.
        (0..255).forEach { v ->
            val accent = Color(0xFF000000L or (v.toLong() shl 16) or (v.toLong() shl 8) or v.toLong())
            val measured = contrastRatio(accentForeground(accent, dark, light), accent)
            assertTrue("grey $v measured $measured", measured >= AA_CONTRAST)
        }
    }

    @Test
    fun theDefaultRedAndTheWarmPresetsKeepTheDarkContentTheyAlreadyHad() {
        // The threshold's answers, arrived at by measurement instead — these must not change.
        assertEquals(dark, on(0xFFE23D3D))   // default Red, luminance ~0.198
        assertEquals(dark, on(0xFFE87B3D))   // an Ember-weight warm accent
    }

    @Test
    fun aDimAccentKeepsTheNearWhiteContent() {
        assertEquals(light, on(0xFF1E3A5F))  // Navy-weight
        assertEquals(light, on(0xFF3D2E1A))
    }

    @Test
    fun theRatioItselfIsSymmetricAndSaneAtTheExtremes() {
        assertEquals(21.0, contrastRatio(Color.Black, Color.White), 0.01)
        assertEquals(21.0, contrastRatio(Color.White, Color.Black), 0.01)
        assertEquals(1.0, contrastRatio(Color.Black, Color.Black), 0.001)
    }
}
