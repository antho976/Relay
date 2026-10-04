package com.tally.app

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import com.tally.app.data.prefs.Accent
import com.tally.app.ui.theme.CategoryPalette
import com.tally.app.ui.theme.TallyError
import com.tally.app.ui.theme.TallySuccess
import com.tally.app.ui.theme.PearlBackground
import com.tally.app.ui.theme.PearlMuted
import com.tally.app.ui.theme.PearlOnBg
import com.tally.app.ui.theme.PearlSurface
import com.tally.app.ui.theme.PearlSurfaceVar
import com.tally.app.ui.theme.accentForeground
import com.tally.app.ui.theme.contrastRatio
import com.tally.app.ui.theme.tallyColorScheme
import com.tally.core.Defaults
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.Locale

/**
 * The palette's promises, measured: text clears 4.5:1 (WCAG AA), marks clear 3:1, and every
 * accent the owner can pick gets a foreground that reads on it.
 */
class ThemeContractTest {

    private fun ratio(value: Double) = String.format(Locale.ROOT, "%.2f:1", value)

    @Test fun mutedTextAtItsFloorAlphaStillReadsOnTheGround() {
        val muted = PearlMuted.copy(alpha = 0.65f).compositeOver(PearlBackground)
        val measured = contrastRatio(muted, PearlBackground)
        assertTrue("PearlMuted at 0.65 on the ground is ${ratio(measured)}", measured >= 4.5)
    }

    @Test fun bodyTextOnTheGroundIsNearMaximum() {
        val measured = contrastRatio(PearlOnBg, PearlBackground)
        assertTrue("PearlOnBg on the ground is ${ratio(measured)}", measured >= 15.0)
    }

    @Test fun fullStrengthTextReadsOnEveryRaisedSurface() {
        listOf("surface" to PearlSurface, "surfaceVariant" to PearlSurfaceVar).forEach { (name, surface) ->
            val body = contrastRatio(PearlOnBg, surface)
            val muted = contrastRatio(PearlMuted, surface)
            assertTrue("PearlOnBg on $name is ${ratio(body)}", body >= 7.0)
            assertTrue("PearlMuted on $name is ${ratio(muted)}", muted >= 4.5)
        }
    }

    @Test fun everyAccentGetsAForegroundThatReads() {
        val failures = Accent.entries.mapNotNull { accent ->
            val fill = Color(accent.argb)
            val measured = contrastRatio(accentForeground(fill, PearlBackground, PearlOnBg), fill)
            if (measured >= 4.5) null else "${accent.label} ${ratio(measured)}"
        }
        assertTrue("Accents whose foreground does not read: $failures", failures.isEmpty())
    }

    @Test fun theSchemeWiresThatForegroundIntoOnPrimary() {
        // Accent off is a near-white neutral; it has to work too.
        val fills = Accent.entries.map { Color(it.argb) } + PearlOnBg
        fills.forEach { fill ->
            listOf(false, true).forEach { amoled ->
                val scheme = tallyColorScheme(fill, amoled)
                val measured = contrastRatio(scheme.onPrimary, scheme.primary)
                assertTrue("onPrimary on $fill (amoled=$amoled) is ${ratio(measured)}", measured >= 4.5)
            }
        }
    }

    @Test fun everyCategoryHueClearsMarkContrastOnTheGround() {
        assertEquals("The palette is the twelve hues categories index into", Defaults.PALETTE_SIZE, CategoryPalette.size)
        val failures = CategoryPalette.mapIndexedNotNull { i, hue ->
            val measured = contrastRatio(hue, PearlBackground)
            if (measured >= 3.0) null else "hue $i ${ratio(measured)}"
        }
        assertTrue("Category hues below 3:1 on the ground: $failures", failures.isEmpty())
    }

    @Test fun stateColoursClearMarkContrast() {
        val error = contrastRatio(TallyError, PearlBackground)
        val errorOnPanel = contrastRatio(TallyError, PearlSurface)
        val success = contrastRatio(TallySuccess, PearlBackground)
        assertTrue("TallyError on the ground is ${ratio(error)}", error >= 3.0)
        assertTrue("TallyError on a panel is ${ratio(errorOnPanel)}", errorOnPanel >= 3.0)
        assertTrue("TallySuccess on the ground is ${ratio(success)}", success >= 3.0)
    }

    @Test fun contrastRatioIsTheWcagFormula() {
        assertEquals(21.0, contrastRatio(Color.White, Color.Black), 0.01)
        assertEquals(1.0, contrastRatio(PearlBackground, PearlBackground), 0.0001)
        assertEquals("It is symmetric", contrastRatio(PearlMuted, PearlBackground), contrastRatio(PearlBackground, PearlMuted), 0.0001)
    }
}
