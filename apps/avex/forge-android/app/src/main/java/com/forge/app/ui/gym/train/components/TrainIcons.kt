package com.forge.app.ui.gym.train.components

import androidx.compose.ui.graphics.vector.ImageVector
import com.forge.app.ui.common.fillPath
import com.forge.app.ui.common.icon
import com.forge.app.ui.common.roundRect
import com.forge.app.ui.common.strokePath

/**
 * The active session's four content glyphs — the row under the set table (NOTE · VIDEO · SWAP ·
 * SKIP) and the collapsed row's trailing demo/swap pair.
 *
 * Drawn in-house for the reason DESIGN §8 gives: content glyphs come from the app's matched
 * families, never Material stock. These four were `Icons.Outlined.Description` / `Visibility` /
 * `SwapHoriz` / `SkipNext`, and four stock Material silhouettes sitting under a screen of custom
 * ones is the single loudest "this came from somewhere else" tell on the train screen.
 *
 * Same construction as [com.forge.app.ui.nav.NavIcons] and
 * [com.forge.app.ui.settings.SettingsIcons]: a 24dp viewport, one visual weight (1.7f strokes plus
 * the odd solid silhouette), rendered muted through `Icon(tint = …)`. Only the builder plumbing is
 * shared with those families — the glyphs themselves stay this screen's own.
 */
object TrainIcons {

    /** Note — a page with three ruled lines, the last one short. */
    val Note: ImageVector by lazy {
        icon("TrainNote") {
            strokePath(1.7f) {
                roundRect(5f, 3.4f, 19f, 20.6f, 2.2f)
                moveTo(8.6f, 8.8f); lineTo(15.4f, 8.8f)
                moveTo(8.6f, 12f); lineTo(15.4f, 12f)
                moveTo(8.6f, 15.2f); lineTo(12.8f, 15.2f)
            }
        }
    }

    /** Video — a play triangle inside the frame it plays in. (It replaced an eye: the row already
     *  says VIDEO, and the action is watching a demo, not revealing something hidden.) */
    val Video: ImageVector by lazy {
        icon("TrainVideo") {
            strokePath(1.7f) { roundRect(2.8f, 5.2f, 21.2f, 18.8f, 2.6f) }
            fillPath {
                moveTo(10.2f, 9.1f)
                lineTo(15.6f, 12f)
                lineTo(10.2f, 14.9f)
                close()
            }
        }
    }

    /** Swap — two tracks running opposite ways: this exercise out, another in. */
    val Swap: ImageVector by lazy {
        icon("TrainSwap") {
            strokePath(1.7f) {
                moveTo(4.4f, 9f); lineTo(19.6f, 9f)
                moveTo(16.2f, 5.6f); lineTo(19.6f, 9f); lineTo(16.2f, 12.4f)
                moveTo(19.6f, 15f); lineTo(4.4f, 15f)
                moveTo(7.8f, 11.6f); lineTo(4.4f, 15f); lineTo(7.8f, 18.4f)
            }
        }
    }

    /** Skip — a play triangle running into the bar that stops it. */
    val Skip: ImageVector by lazy {
        icon("TrainSkip") {
            fillPath {
                moveTo(6.2f, 5.8f)
                lineTo(15.2f, 12f)
                lineTo(6.2f, 18.2f)
                close()
            }
            strokePath(2f) { moveTo(18.2f, 5.8f); lineTo(18.2f, 18.2f) }
        }
    }
}
