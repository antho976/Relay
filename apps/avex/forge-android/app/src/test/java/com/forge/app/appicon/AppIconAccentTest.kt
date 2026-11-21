package com.forge.app.appicon

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** "Match accent to icon": every icon with a colour yields an accent that reads on the background. */
class AppIconAccentTest {

    private fun luminance(rgb: Int): Double {
        fun lin(c: Int): Double = (c / 255.0).let { if (it <= 0.04045) it / 12.92 else Math.pow((it + 0.055) / 1.055, 2.4) }
        return 0.2126 * lin((rgb shr 16) and 0xFF) + 0.7152 * lin((rgb shr 8) and 0xFF) + 0.0722 * lin(rgb and 0xFF)
    }

    @Test
    fun everyColouredIconReadsOnTheBackground() {
        val bg = luminance(0x110F0C)
        AppIcon.entries.filter { it.launchPalette != null }.forEach { icon ->
            val hex = icon.accentHex!!
            val ratio = (luminance(hex.removePrefix("#").toInt(16)) + 0.05) / (bg + 0.05)
            assertTrue("${icon.name} $hex measures $ratio", ratio >= 3.0)
        }
    }

    @Test
    fun aBrightIconKeepsItsOwnColour() {
        assertEquals("#38C4DC", AppIcon.StealthCyan.accentHex)
        assertEquals("#D93636", AppIcon.SolidRed.accentHex)
    }

    @Test
    fun theHouseIconsHaveNone() {
        assertNull(AppIcon.Default.accentHex)
        assertNull(AppIcon.AvexSignal.accentHex)
    }
}
