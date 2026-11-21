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
 * Custom glyphs for the five primary [BottomTab]s, plus [Profile], which left the bar for the
 * Home top bar on 2026-07-27 but keeps its glyph here with the family it was drawn to match. Material's stock set reads as
 * generic (the running-man for Cardio and the brain-with-gears for Coach in
 * particular), so the whole bar is drawn as one matched family instead:
 *
 *  - [Cardio]  — a bare ECG/heartbeat trace (no figure, no heart shape).
 *  - [Stats]   — three rounded bars rising left→right (a growth/chart read).
 *  - [Home]    — a pitched-roof house with a doorway notch.
 *  - [Coach]   — a compass: an outline ring with a NE-pointing needle.
 *  - [Academy] — an open book: two leaves meeting at a spine.
 *  - [Profile] — a head + shoulders bust (no enclosing ring, so it stays
 *    distinct from the Coach compass). No longer a tab; it leads the Home
 *    top bar's profile entry.
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

    /** Academy — an open book: two leaves meeting at a spine, with a page line on each. */
    val Academy: ImageVector by lazy {
        navIcon("NavAcademy") {
            strokePath(width = 1.9f) {
                // Left leaf: down the spine, out along the base, up the fore-edge to the top corner.
                moveTo(12f, 7.2f)
                curveTo(10.2f, 5.4f, 7.4f, 4.6f, 3.4f, 4.8f)
                lineTo(3.4f, 17.6f)
                curveTo(7.4f, 17.4f, 10.2f, 18.2f, 12f, 20f)
                // Right leaf mirrors it, so the spine reads as one continuous line.
                curveTo(13.8f, 18.2f, 16.6f, 17.4f, 20.6f, 17.6f)
                lineTo(20.6f, 4.8f)
                curveTo(16.6f, 4.6f, 13.8f, 5.4f, 12f, 7.2f)
                close()
                // The spine itself.
                moveTo(12f, 7.2f)
                lineTo(12f, 20f)
            }
        }
    }

    /**
     * Head + shoulders bust, drawn for the TOP BAR rather than the tab bar (2026-07-27).
     *
     * The nav-bar version was wide and bottom-heavy with its shoulders chopped flat by the viewport
     * edge — which is right above a text label, where a glyph reads as a silhouette. Beside the
     * notification bell it read as squat and cut off. This one is narrower (12.2 units across, near
     * the bell's own width), lifts off the baseline, and rounds its base corners so it reads as a
     * finished mark rather than a cropped one.
     */
    val Profile: ImageVector by lazy {
        navIcon("NavProfile") {
            fillPath {
                circle(12f, 7.6f, 3.6f) // head
            }
            fillPath {
                // Shoulders: a dome that lands on softly rounded base corners, clear of the edge.
                moveTo(5.9f, 18.4f)
                curveTo(5.9f, 14.6f, 8.6f, 12.8f, 12f, 12.8f)
                curveTo(15.4f, 12.8f, 18.1f, 14.6f, 18.1f, 18.4f)
                lineTo(18.1f, 18.7f)
                quadTo(18.1f, 20.1f, 16.7f, 20.1f)
                lineTo(7.3f, 20.1f)
                quadTo(5.9f, 20.1f, 5.9f, 18.7f)
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
