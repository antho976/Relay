package com.forge.app.ui.nav

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
 * Custom glyphs for the five primary [BottomTab]s. Material's stock set reads as
 * generic (the running-man for Cardio and the brain-with-gears for Coach in
 * particular), so the whole bar is drawn as one matched family instead:
 *
 *  - [Cardio]  — a bare ECG/heartbeat trace (no figure, no heart shape).
 *  - [Stats]   — three rounded bars rising left→right (a growth/chart read).
 *  - [Home]    — a pitched-roof house with a doorway notch.
 *  - [Coach]   — a compass: an outline ring with a NE-pointing needle.
 *  - [Profile] — a head + shoulders bust (no enclosing ring, so it stays
 *    distinct from the Coach compass).
 *
 * All five are 24dp glyphs at a single visual weight (filled silhouettes, except
 * Cardio which is a stroked line). The source paint is black; callers render them
 * through `Icon(..., tint = ...)`, which recolors the whole vector (fill and
 * stroke alike) exactly like the `Icons.Filled.*` glyphs did.
 */
object NavIcons {

    /** Bare heartbeat trace: a stroked baseline with one sharp peak then trough, round joints. */
    val Cardio: ImageVector by lazy {
        navIcon("NavCardio") {
            strokePath(width = 2.2f) {
                moveTo(2.5f, 12f)
                lineTo(8.5f, 12f)
                lineTo(11f, 6.5f)
                lineTo(13f, 17.5f)
                lineTo(15.5f, 12f)
                lineTo(21.5f, 12f)
            }
        }
    }

    /** Three bars rising left→right, sharing a baseline, with rounded top corners. */
    val Stats: ImageVector by lazy {
        navIcon("NavStats") {
            fillPath {
                topRoundedBar(cx = 5.4f, halfWidth = 1.6f, top = 12.5f, bottom = 20f, cornerR = 1f)
                topRoundedBar(cx = 12f, halfWidth = 1.6f, top = 8.5f, bottom = 20f, cornerR = 1f)
                topRoundedBar(cx = 18.6f, halfWidth = 1.6f, top = 4.5f, bottom = 20f, cornerR = 1f)
            }
        }
    }

    /** Pitched-roof house with a doorway notch carved out (even-odd). */
    val Home: ImageVector by lazy {
        navIcon("NavHome") {
            fillPath(PathFillType.EvenOdd) {
                // House outline: apex, down both roof slopes, down the walls.
                moveTo(12f, 2.8f)
                lineTo(21f, 10.8f)
                lineTo(21f, 21f)
                lineTo(3f, 21f)
                lineTo(3f, 10.8f)
                close()
                // Doorway notch (carved by even-odd fill).
                moveTo(9.8f, 14.2f)
                lineTo(14.2f, 14.2f)
                lineTo(14.2f, 21f)
                lineTo(9.8f, 21f)
                close()
            }
        }
    }

    /** Compass: outline ring + the classic twisted pinwheel needle. */
    val Coach: ImageVector by lazy {
        navIcon("NavCoach") {
            // Outline ring (outer minus inner).
            fillPath(PathFillType.EvenOdd) {
                circle(12f, 12f, 9f)
                circle(12f, 12f, 7.3f)
            }
            // Pinwheel needle: SE waist → SW tip → NW waist → NE tip.
            fillPath {
                moveTo(13.36f, 13.36f)
                lineTo(8.28f, 15.72f)
                lineTo(10.64f, 10.64f)
                lineTo(15.72f, 8.28f)
                close()
            }
        }
    }

    /** Head + shoulders bust. */
    val Profile: ImageVector by lazy {
        navIcon("NavProfile") {
            fillPath {
                circle(12f, 8.6f, 3.7f) // head
            }
            fillPath {
                // Shoulders: a rounded bust rising from the baseline to a domed top.
                moveTo(4.6f, 21.2f)
                curveTo(4.6f, 16.0f, 7.8f, 13.7f, 12f, 13.7f)
                curveTo(16.2f, 13.7f, 19.4f, 16.0f, 19.4f, 21.2f)
                lineTo(4.6f, 21.2f)
                close()
            }
        }
    }
}

// --- builders --------------------------------------------------------------

private fun navIcon(name: String, block: ImageVector.Builder.() -> Unit): ImageVector =
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

/** Full circle centered at ([cx], [cy]) with radius [r]. */
private fun PathBuilder.circle(cx: Float, cy: Float, r: Float) {
    moveTo(cx - r, cy)
    arcToRelative(r, r, 0f, true, true, 2 * r, 0f)
    arcToRelative(r, r, 0f, true, true, -2 * r, 0f)
    close()
}

/** A stroked path with round caps and joins — for line glyphs (no fill, no overlap artifacts). */
private fun ImageVector.Builder.strokePath(width: Float, block: PathBuilder.() -> Unit) {
    path(
        stroke = SolidColor(Color.Black),
        strokeLineWidth = width,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
        pathBuilder = block,
    )
}

/** A vertical bar centered at [cx], half-width [halfWidth], with rounded top corners and a flat base. */
private fun PathBuilder.topRoundedBar(
    cx: Float,
    halfWidth: Float,
    top: Float,
    bottom: Float,
    cornerR: Float,
) {
    val left = cx - halfWidth
    val right = cx + halfWidth
    moveTo(left, bottom)
    lineTo(left, top + cornerR)
    quadTo(left, top, left + cornerR, top)
    lineTo(right - cornerR, top)
    quadTo(right, top, right, top + cornerR)
    lineTo(right, bottom)
    close()
}
