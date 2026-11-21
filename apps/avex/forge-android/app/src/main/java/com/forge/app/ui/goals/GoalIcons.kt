package com.forge.app.ui.goals

import androidx.compose.ui.graphics.vector.ImageVector
import com.forge.app.ui.common.circle
import com.forge.app.ui.common.fillPath
import com.forge.app.ui.common.icon
import com.forge.app.ui.common.roundRect
import com.forge.app.ui.common.strokePath

/**
 * Glyphs for goal kinds that had no mark of their own (2026-09-27). Cardio distance and cardio time
 * both wore the cardio tab's pulse and Workouts wore the session clock, so the add-a-goal chooser
 * showed two identical tiles and a clock that read as "time". Drawn at the settings families' 1.8
 * stroke on the shared 24dp grid, so they sit beside [com.forge.app.ui.nav.NavIcons.Stats] and the
 * exercise glyphs without a weight jump.
 */
internal object GoalIcons {
    private const val W = 1.8f

    /** Cardio distance: a route from a start dot to a ringed finish. */
    val Route: ImageVector by lazy {
        icon("GoalRoute") {
            strokePath(W) {
                moveTo(6.0f, 17.0f)
                curveTo(10.5f, 17.0f, 8.0f, 11.0f, 12.0f, 11.0f)
                curveTo(16.0f, 11.0f, 13.5f, 6.6f, 17.6f, 6.6f)
            }
            fillPath { circle(5.2f, 17.4f, 2.2f) }
            strokePath(W) { circle(18.4f, 6.4f, 2.4f) }
        }
    }

    /** Cardio time: a stopwatch. */
    val Stopwatch: ImageVector by lazy {
        icon("GoalStopwatch") {
            strokePath(W) {
                circle(12f, 13.6f, 7.4f)
                moveTo(10.0f, 3.2f); lineTo(14.0f, 3.2f)
                moveTo(12.0f, 3.2f); lineTo(12.0f, 6.2f)
                moveTo(18.0f, 6.8f); lineTo(19.4f, 5.4f)
                moveTo(12.0f, 13.6f); lineTo(14.8f, 10.8f)
            }
            fillPath { circle(12f, 13.6f, 1.3f) }
        }
    }

    /** Workouts: a calendar with a day ticked off. */
    val CalendarCheck: ImageVector by lazy {
        icon("GoalCalendar") {
            strokePath(W) {
                roundRect(3.8f, 5.4f, 20.2f, 20.4f, 2.4f)
                moveTo(3.8f, 9.8f); lineTo(20.2f, 9.8f)
                moveTo(8.4f, 3.2f); lineTo(8.4f, 7.0f)
                moveTo(15.6f, 3.2f); lineTo(15.6f, 7.0f)
                moveTo(8.6f, 15.0f); lineTo(11.0f, 17.4f); lineTo(15.6f, 12.8f)
            }
        }
    }

    /** Bodyweight: a bathroom scale and its dial. */
    val Scale: ImageVector by lazy {
        icon("GoalScale") {
            strokePath(W) {
                roundRect(3.6f, 3.6f, 20.4f, 20.4f, 4.0f)
                moveTo(7.8f, 11.0f); curveTo(7.8f, 5.4f, 16.2f, 5.4f, 16.2f, 11.0f)
                moveTo(12.0f, 10.6f); lineTo(13.8f, 7.6f)
            }
        }
    }
}
