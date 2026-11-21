package com.forge.app.ui.notifications

import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.vector.ImageVector
import com.forge.app.data.repo.NoticeGlyph
import com.forge.app.ui.common.circle
import com.forge.app.ui.common.fillPath
import com.forge.app.ui.common.icon
import com.forge.app.ui.common.roundRect
import com.forge.app.ui.common.strokePath
import com.forge.app.ui.nav.NavIcons

/**
 * Leading glyphs for the notifications feed — the same matched family and single visual weight as
 * [NavIcons] and [com.forge.app.ui.settings.SettingsIcons] (24dp viewport, stroked outlines with the
 * odd filled accent), rendered muted through `Icon(tint = ...)`. Drawn in-house rather than pulled
 * from Material stock for the same reason the bottom bar is (DESIGN §8: content glyphs come from the
 * custom families, never stock).
 *
 * The coach brief reuses `NavIcons.Coach` outright, so a coach row speaks the same glyph here, in
 * the bottom bar and in Settings.
 */
object NoticeIcons {

    /** Unfinished workout — a progress ring broken at the top right, with the arc's head filled. */
    private val Session: ImageVector by lazy {
        icon("NoticeSession") {
            strokePath(1.9f) {
                // Open arc: starts past 12 o'clock and sweeps the long way round, leaving a gap.
                moveTo(15.6f, 4.6f)
                arcToRelative(8.3f, 8.3f, 0f, true, true, -5.1f, -0.8f)
            }
            fillPath { circle(15.6f, 4.6f, 1.9f) }
        }
    }

    /** Milestone — a four-point star, the same mark the milestone lines have always carried. */
    private val Milestone: ImageVector by lazy {
        icon("NoticeMilestone") {
            fillPath {
                // Concave diamond: each quadrant curves back toward the centre, giving four points.
                moveTo(12f, 2.4f)
                quadTo(13.2f, 9.4f, 21.6f, 12f)
                quadTo(13.2f, 14.6f, 12f, 21.6f)
                quadTo(10.8f, 14.6f, 2.4f, 12f)
                quadTo(10.8f, 9.4f, 12f, 2.4f)
                close()
            }
        }
    }

    /** Import — an arrow landing in an open tray. */
    private val Import: ImageVector by lazy {
        icon("NoticeImport") {
            strokePath(1.9f) {
                moveTo(12f, 3.2f)
                lineTo(12f, 13.4f)
                moveTo(7.8f, 9.4f)
                lineTo(12f, 13.6f)
                lineTo(16.2f, 9.4f)
                // Tray: down each side and across the floor.
                moveTo(4.2f, 15.2f)
                lineTo(4.2f, 20f)
                lineTo(19.8f, 20f)
                lineTo(19.8f, 15.2f)
            }
        }
    }

    /** Backup restored — a shield with a filled tick punched out of it. */
    private val Backup: ImageVector by lazy {
        icon("NoticeBackup") {
            strokePath(1.9f) {
                moveTo(12f, 2.8f)
                lineTo(19.4f, 5.9f)
                lineTo(19.4f, 11.6f)
                // Both flanks sweep down to a single point at the base.
                curveTo(19.4f, 16.4f, 16.2f, 19.5f, 12f, 21.2f)
                curveTo(7.8f, 19.5f, 4.6f, 16.4f, 4.6f, 11.6f)
                lineTo(4.6f, 5.9f)
                close()
                moveTo(8.6f, 11.8f)
                lineTo(11.1f, 14.3f)
                lineTo(15.6f, 9.2f)
            }
        }
    }

    /** Housekeeping — a sweep: three trailing strokes behind a filled dot. */
    private val Housekeeping: ImageVector by lazy {
        icon("NoticeHousekeeping") {
            fillPath { circle(17.4f, 7.2f, 2.6f) }
            strokePath(1.9f) {
                moveTo(13.2f, 11.6f)
                lineTo(5.4f, 19.4f)
                moveTo(17f, 13.6f)
                lineTo(12.6f, 18f)
                moveTo(19.6f, 16.8f)
                lineTo(17.8f, 18.6f)
            }
        }
    }

    /** Turn on notifications — a bell, matching the one in every top bar. Also the options sheet's
     *  "Notification settings" row, so the two speak one glyph. */
    val Bell: ImageVector by lazy {
        icon("NoticeBell") {
            strokePath(1.9f) {
                // Dome over a flat lip: the classic bell silhouette, drawn open at the base.
                moveTo(5.4f, 17.2f)
                lineTo(5.4f, 10.8f)
                arcToRelative(6.6f, 6.6f, 0f, true, true, 13.2f, 0f)
                lineTo(18.6f, 17.2f)
                close()
                moveTo(10.2f, 20f)
                lineTo(13.8f, 20f)
            }
        }
    }

    /** Connect a watch — a case with a strap stub above and below, and a filled crown. */
    private val Watch: ImageVector by lazy {
        icon("NoticeWatch") {
            strokePath(1.9f) {
                roundRect(6.4f, 7.2f, 17.6f, 16.8f, 2.4f)
                moveTo(9.2f, 7.2f)
                lineTo(9.6f, 3.4f)
                lineTo(14.4f, 3.4f)
                lineTo(14.8f, 7.2f)
                moveTo(9.2f, 16.8f)
                lineTo(9.6f, 20.6f)
                lineTo(14.4f, 20.6f)
                lineTo(14.8f, 16.8f)
            }
            fillPath(PathFillType.NonZero) { roundRect(17.6f, 10.4f, 19.4f, 13.6f, 0.9f) }
        }
    }

    /** Clear all — a bin: lid over a tapered body with two staves. Options sheet only. */
    val Trash: ImageVector by lazy {
        icon("NoticeTrash") {
            strokePath(1.9f) {
                moveTo(4.2f, 6.6f)
                lineTo(19.8f, 6.6f)
                // Lid handle.
                moveTo(9.4f, 6.6f)
                lineTo(9.4f, 4.2f)
                lineTo(14.6f, 4.2f)
                lineTo(14.6f, 6.6f)
                // Body, tapering slightly toward the base.
                moveTo(6.2f, 6.6f)
                lineTo(7.1f, 20.2f)
                lineTo(16.9f, 20.2f)
                lineTo(17.8f, 6.6f)
                // Staves.
                moveTo(10.4f, 10.2f)
                lineTo(10.7f, 16.8f)
                moveTo(13.6f, 10.2f)
                lineTo(13.3f, 16.8f)
            }
        }
    }

    /** The glyph a row leads with. Coach reuses the hub's compass rather than inventing a second one. */
    fun forGlyph(glyph: NoticeGlyph): ImageVector = when (glyph) {
        NoticeGlyph.SESSION -> Session
        NoticeGlyph.COACH -> NavIcons.Coach
        NoticeGlyph.MILESTONE -> Milestone
        NoticeGlyph.IMPORT -> Import
        NoticeGlyph.BACKUP -> Backup
        NoticeGlyph.HOUSEKEEPING -> Housekeeping
        NoticeGlyph.BELL -> Bell
        NoticeGlyph.WATCH -> Watch
        // The same glyph the Academy tab carries, so a lesson row is recognisably from there.
        NoticeGlyph.ACADEMY -> NavIcons.Academy
    }
}
