package com.forge.app.ui.coach

import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.vector.ImageVector
import com.forge.app.ui.common.circle
import com.forge.app.ui.common.fillPath
import com.forge.app.ui.common.icon
import com.forge.app.ui.common.strokePath

/**
 * Leading glyphs for the coach's input rows — the same matched family and single visual weight as
 * [com.forge.app.ui.nav.NavIcons] and `SettingsIcons` (24dp viewport, filled silhouettes plus the
 * odd stroked line), rendered muted so they read as wayfinding rather than decoration.
 *
 * These earn their place on exactly one list: the inputs the coach reads. That list is four rows of
 * near-identical shape on the emptiest screen a new account sees, and a glyph is what lets the eye
 * find "sleep" without reading all four. Content everywhere else on this page stays text-first.
 */
internal object CoachIcons {

    /** Sleep — a crescent moon: a disc with an offset disc CONTAINED inside it, so even-odd truly
     *  subtracts (an offset that pokes out would XOR into a lumpy blob). */
    val Sleep: ImageVector by lazy {
        icon("CoachSleep") {
            fillPath(PathFillType.EvenOdd) {
                circle(12f, 12f, 8.3f)
                circle(13.2f, 10.8f, 6.3f)
            }
        }
    }

    /** Resting heart rate — a pulse trace crossing the full width, with one tall spike. */
    val Heart: ImageVector by lazy {
        icon("CoachHeart") {
            strokePath(1.9f) {
                moveTo(2.5f, 12f)
                lineTo(7f, 12f)
                lineTo(9.2f, 6.4f)
                lineTo(12.4f, 17.6f)
                lineTo(15f, 12f)
                lineTo(21.5f, 12f)
            }
        }
    }

    /** Effort check-ins — a filled disc with a rising bar beside it: how hard a session felt. */
    val Effort: ImageVector by lazy {
        icon("CoachEffort") {
            fillPath {
                circle(7.2f, 12f, 3.4f)
            }
            strokePath(1.9f) {
                moveTo(13.6f, 15.6f); lineTo(13.6f, 12.4f)
                moveTo(17.2f, 15.6f); lineTo(17.2f, 9.6f)
                moveTo(20.8f, 15.6f); lineTo(20.8f, 6.8f)
            }
        }
    }

    /** Rest-day flags — a pennant on a staff: a day deliberately marked. */
    val RestDay: ImageVector by lazy {
        icon("CoachRestDay") {
            strokePath(1.9f) {
                moveTo(6.4f, 3.6f); lineTo(6.4f, 20.4f)
            }
            fillPath {
                moveTo(6.4f, 4.6f)
                lineTo(18.4f, 8.2f)
                lineTo(6.4f, 11.8f)
                close()
            }
        }
    }

    /** Anything the registry adds later, until it earns a drawn glyph of its own: a hollow ring. */
    val Generic: ImageVector by lazy {
        icon("CoachGeneric") {
            strokePath(1.8f) { circle(12f, 12f, 7.4f) }
        }
    }

    /**
     * The glyph for a recovery input, matched on the repository's own label. An unmatched label
     * falls through to [Generic] rather than to no glyph, so the row's leading column never goes
     * ragged when a new signal ships.
     */
    fun forSignal(label: String): ImageVector = when {
        label.equals("Sleep", ignoreCase = true) -> Sleep
        label.contains("heart", ignoreCase = true) -> Heart
        label.contains("effort", ignoreCase = true) -> Effort
        label.contains("rest-day", ignoreCase = true) || label.contains("rest day", ignoreCase = true) -> RestDay
        else -> Generic
    }
}
