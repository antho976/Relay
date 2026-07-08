package com.forge.app.ui.settings

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathBuilder
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp

/**
 * Small leading glyphs for the settings nav rows — the SAME matched family and single visual weight
 * as [com.forge.app.ui.nav.NavIcons] (24dp viewport, filled silhouettes + the odd stroked line),
 * rendered muted through `Icon(tint = ...)` so they read as quiet wayfinding, not decoration. Drawn
 * in-house for the same reason the bottom bar is: Material's stock set looks generic and off-family.
 * The Coach row reuses `NavIcons.Coach` directly, so Settings and the hub speak one glyph.
 */
object SettingsIcons {

    /** Appearance — a half-toned disc (light/dark theme): outline ring + a filled right half. */
    val Appearance: ImageVector by lazy {
        icon("SetAppearance") {
            strokePath(1.7f) { circle(12f, 12f, 8.2f) }
            fillPath {
                moveTo(12f, 3.8f)
                arcToRelative(8.2f, 8.2f, 0f, false, true, 0f, 16.4f)
                close()
            }
        }
    }

    /** Units & format — a ruler: a rounded bar with three descending tick marks. */
    val Units: ImageVector by lazy {
        icon("SetUnits") {
            strokePath(1.7f) {
                roundRect(3.3f, 8.8f, 20.7f, 15.2f, 2f)
                moveTo(8f, 8.8f); lineTo(8f, 12f)
                moveTo(12f, 8.8f); lineTo(12f, 12.8f)
                moveTo(16f, 8.8f); lineTo(16f, 12f)
            }
        }
    }

    /** Notifications — a bell: filled body with a top knob and a clapper bump fused to the rim
     *  (a detached clapper dot reads as scuffed at this size). */
    val Notifications: ImageVector by lazy {
        icon("SetNotifications") {
            fillPath { circle(12f, 3.7f, 1.1f) }   // knob
            fillPath {
                moveTo(12f, 5f)
                curveTo(8.6f, 5f, 7.6f, 7.6f, 7.6f, 11.2f)
                curveTo(7.6f, 14.4f, 6.6f, 15.9f, 5.8f, 16.8f)
                lineTo(18.2f, 16.8f)
                curveTo(17.4f, 15.9f, 16.4f, 14.4f, 16.4f, 11.2f)
                curveTo(16.4f, 7.6f, 15.4f, 5f, 12f, 5f)
                close()
            }
            fillPath {
                moveTo(10.3f, 16.8f)
                curveTo(10.3f, 18.8f, 13.7f, 18.8f, 13.7f, 16.8f)
                close()
            }
        }
    }

    /** Program & equipment — a clipboard plan: stroked board, filled clip tab, and three plan
     *  lines (the third shorter, so it reads as a program being written, not a text block). */
    val Program: ImageVector by lazy {
        icon("SetProgram") {
            strokePath(1.7f) { roundRect(5.2f, 4.6f, 18.8f, 20.2f, 2f) }
            fillPath { roundRect(9.0f, 2.9f, 15.0f, 6.3f, 1.2f) }
            strokePath(1.7f) {
                moveTo(8.6f, 10.8f); lineTo(15.4f, 10.8f)
                moveTo(8.6f, 14.2f); lineTo(15.4f, 14.2f)
                moveTo(8.6f, 17.6f); lineTo(12.4f, 17.6f)
            }
        }
    }

    /** Session — a clock (rest timer / pacing): a face ring, a hub, and two hands. */
    val Session: ImageVector by lazy {
        icon("SetSession") {
            strokePath(1.8f) { circle(12f, 12.4f, 7.7f) }
            fillPath { circle(12f, 12.4f, 1.0f) }
            strokePath(1.8f) {
                moveTo(12f, 12.4f); lineTo(12f, 7.8f)     // long hand, up
                moveTo(12f, 12.4f); lineTo(15.0f, 13.4f)  // short hand, ~4 o'clock
            }
        }
    }

    /** Exercise likes — a heart (clean symmetric bezier). */
    val Likes: ImageVector by lazy {
        icon("SetLikes") {
            fillPath {
                moveTo(12f, 20f)
                curveTo(6.8f, 15.8f, 3.5f, 12.8f, 3.5f, 9.2f)
                curveTo(3.5f, 6.9f, 5.4f, 5.2f, 7.6f, 5.2f)
                curveTo(9.4f, 5.2f, 11.1f, 6.4f, 12f, 8f)
                curveTo(12.9f, 6.4f, 14.6f, 5.2f, 16.4f, 5.2f)
                curveTo(18.6f, 5.2f, 20.5f, 6.9f, 20.5f, 9.2f)
                curveTo(20.5f, 12.8f, 17.2f, 15.8f, 12f, 20f)
                close()
            }
        }
    }

    /** Recovery — a crescent moon (sleep / rest): a disc with an offset disc CONTAINED inside it,
     *  so even-odd truly subtracts (an offset that pokes out would XOR into a lumpy blob). */
    val Recovery: ImageVector by lazy {
        icon("SetRecovery") {
            fillPath(PathFillType.EvenOdd) {
                circle(12f, 12f, 8.3f)
                circle(13.2f, 10.8f, 6.3f)
            }
        }
    }

    /** Holiday / Vacation — a sun: a filled core with eight rays. */
    val Holiday: ImageVector by lazy {
        icon("SetHoliday") {
            fillPath { circle(12f, 12f, 4f) }
            strokePath(1.8f) {
                moveTo(18f, 12f); lineTo(20.6f, 12f)
                moveTo(6f, 12f); lineTo(3.4f, 12f)
                moveTo(12f, 6f); lineTo(12f, 3.4f)
                moveTo(12f, 18f); lineTo(12f, 20.6f)
                moveTo(16.24f, 7.76f); lineTo(18.08f, 5.92f)
                moveTo(7.76f, 16.24f); lineTo(5.92f, 18.08f)
                moveTo(16.24f, 16.24f); lineTo(18.08f, 18.08f)
                moveTo(7.76f, 7.76f); lineTo(5.92f, 5.92f)
            }
        }
    }

    /** Export data — an arrow saving down into an open tray. */
    val Export: ImageVector by lazy {
        icon("SetExport") {
            strokePath(1.8f) {
                moveTo(4.6f, 14f); lineTo(4.6f, 19f); lineTo(19.4f, 19f); lineTo(19.4f, 14f)
                moveTo(12f, 4f); lineTo(12f, 14.6f)
                moveTo(8f, 10.8f); lineTo(12f, 14.8f); lineTo(16f, 10.8f)
            }
        }
    }

    /** Import data — the mirror of [Export]: an arrow rising UP out of an open tray (data into the app). */
    val Import: ImageVector by lazy {
        icon("SetImport") {
            strokePath(1.8f) {
                moveTo(4.6f, 14f); lineTo(4.6f, 19f); lineTo(19.4f, 19f); lineTo(19.4f, 14f)
                moveTo(12f, 15f); lineTo(12f, 4.4f)
                moveTo(8f, 8.2f); lineTo(12f, 4.2f); lineTo(16f, 8.2f)
            }
        }
    }
}

// --- builders (mirrors NavIcons, kept local so the two families evolve independently) -------------

private fun icon(name: String, block: ImageVector.Builder.() -> Unit): ImageVector =
    ImageVector.Builder(
        name = name,
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).apply(block).build()

private fun ImageVector.Builder.fillPath(
    fillType: PathFillType = PathFillType.NonZero,
    block: PathBuilder.() -> Unit,
) {
    path(fill = SolidColor(Color.Black), pathFillType = fillType, pathBuilder = block)
}

private fun ImageVector.Builder.strokePath(width: Float, block: PathBuilder.() -> Unit) {
    path(
        stroke = SolidColor(Color.Black),
        strokeLineWidth = width,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
        pathBuilder = block,
    )
}

/** Full circle centered at ([cx], [cy]) with radius [r]. */
private fun PathBuilder.circle(cx: Float, cy: Float, r: Float) {
    moveTo(cx - r, cy)
    arcToRelative(r, r, 0f, true, true, 2 * r, 0f)
    arcToRelative(r, r, 0f, true, true, -2 * r, 0f)
    close()
}

/** A rounded rectangle from ([l],[t]) to ([r],[b]) with corner radius [rad]. */
private fun PathBuilder.roundRect(l: Float, t: Float, r: Float, b: Float, rad: Float) {
    moveTo(l + rad, t)
    lineTo(r - rad, t)
    quadTo(r, t, r, t + rad)
    lineTo(r, b - rad)
    quadTo(r, b, r - rad, b)
    lineTo(l + rad, b)
    quadTo(l, b, l, b - rad)
    lineTo(l, t + rad)
    quadTo(l, t, l + rad, t)
    close()
}
