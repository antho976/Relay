package com.forge.app.ui.common

import androidx.compose.ui.graphics.vector.ImageVector
import com.forge.app.program.Equipment

/**
 * Equipment glyphs for exercise rows (Exercise likes, pickers) — the SAME matched family and visual
 * weight as [com.forge.app.ui.nav.NavIcons] / SettingsIcons (24dp viewport, filled silhouettes plus
 * the odd stroked line), rendered muted via `Icon(tint = ...)` so they read as quiet wayfinding.
 * One glyph per equipment CLASS, not per station — [forEquipment] collapses the enum (all bars →
 * [Barbell], both benches → [Bench]) so the family stays small and each shape stays recognizable
 * at 20dp.
 */
object ExerciseIcons {

    /** Barbell — a long bar through two filled plates. Also stands in for EZ/trap bars and racks. */
    val Barbell: ImageVector by lazy {
        icon("ExBarbell") {
            strokePath(1.8f) { moveTo(2.8f, 12f); lineTo(21.2f, 12f) }
            fillPath { roundRect(5.6f, 7.6f, 8.0f, 16.4f, 1f) }
            fillPath { roundRect(16.0f, 7.6f, 18.4f, 16.4f, 1f) }
        }
    }

    /** Dumbbell — a short handle, big inner plates, small end caps. */
    val Dumbbell: ImageVector by lazy {
        icon("ExDumbbell") {
            strokePath(1.8f) { moveTo(8.4f, 12f); lineTo(15.6f, 12f) }
            fillPath { roundRect(5.4f, 7.2f, 8.4f, 16.8f, 1.2f) }
            fillPath { roundRect(15.6f, 7.2f, 18.6f, 16.8f, 1.2f) }
            fillPath { roundRect(3.4f, 9.4f, 5.4f, 14.6f, 0.9f) }
            fillPath { roundRect(18.6f, 9.4f, 20.6f, 14.6f, 0.9f) }
        }
    }

    /** Kettlebell — a filled bell body under a stroked handle arc. */
    val Kettlebell: ImageVector by lazy {
        icon("ExKettlebell") {
            strokePath(1.8f) {
                moveTo(8.8f, 11.2f)
                curveTo(8.8f, 5.8f, 15.2f, 5.8f, 15.2f, 11.2f)
            }
            fillPath { circle(12f, 14.4f, 5.3f) }
        }
    }

    /** Cable machine — a top pulley wheel with the cable dropping to a filled handle bar. */
    val Cable: ImageVector by lazy {
        icon("ExCable") {
            strokePath(1.7f) { circle(12f, 6.2f, 2.6f) }
            strokePath(1.7f) { moveTo(12f, 8.8f); lineTo(12f, 16.2f) }
            fillPath { roundRect(8.4f, 16.2f, 15.6f, 18.4f, 1f) }
        }
    }

    /** Machine — a selectorized weight stack: framed plates with the pin cable rising off the top. */
    val Machine: ImageVector by lazy {
        icon("ExMachine") {
            strokePath(1.7f) { roundRect(6.8f, 7.2f, 17.2f, 19f, 1.5f) }
            strokePath(1.7f) {
                moveTo(6.8f, 11.2f); lineTo(17.2f, 11.2f)
                moveTo(6.8f, 15.1f); lineTo(17.2f, 15.1f)
            }
            strokePath(1.7f) { moveTo(12f, 7.2f); lineTo(12f, 3.8f) }
        }
    }

    /** Resistance band — a stroked loop with two free ends trailing off the bottom. */
    val Band: ImageVector by lazy {
        icon("ExBand") {
            strokePath(1.8f) { circle(12f, 10.4f, 5.4f) }
            strokePath(1.8f) {
                moveTo(8.6f, 14.6f)
                curveTo(7.2f, 17.2f, 5.8f, 18.6f, 4.4f, 19.6f)
                moveTo(15.4f, 14.6f)
                curveTo(16.8f, 17.2f, 18.2f, 18.6f, 19.6f, 19.6f)
            }
        }
    }

    /** Pull-up bar — a figure hanging from the bar: V arms, filled head, trailing legs. */
    val PullUpBar: ImageVector by lazy {
        icon("ExPullUp") {
            strokePath(1.8f) { moveTo(3.6f, 4.8f); lineTo(20.4f, 4.8f) }
            strokePath(1.7f) {
                moveTo(8.8f, 4.8f); lineTo(11.2f, 8.4f)
                moveTo(15.2f, 4.8f); lineTo(12.8f, 8.4f)
            }
            fillPath { circle(12f, 10.2f, 1.9f) }
            strokePath(1.7f) {
                moveTo(12f, 12.1f); lineTo(12f, 15.8f)
                moveTo(12f, 15.8f); lineTo(10.4f, 19.4f)
                moveTo(12f, 15.8f); lineTo(13.6f, 19.4f)
            }
        }
    }

    /** Bench — a side-on pad on two legs. */
    val Bench: ImageVector by lazy {
        icon("ExBench") {
            fillPath { roundRect(3.4f, 9.2f, 20.6f, 12.4f, 1.2f) }
            strokePath(1.8f) {
                moveTo(6.6f, 12.4f); lineTo(6.6f, 17.6f)
                moveTo(17.4f, 12.4f); lineTo(17.4f, 17.6f)
            }
        }
    }

    /** Bodyweight — a spread-limb figure: filled head, stroked trunk, arms, and legs. */
    val Bodyweight: ImageVector by lazy {
        icon("ExBodyweight") {
            fillPath { circle(12f, 5.4f, 2.1f) }
            strokePath(1.7f) {
                moveTo(12f, 7.8f); lineTo(12f, 14.2f)
                moveTo(7f, 12.4f); lineTo(12f, 9.4f); lineTo(17f, 12.4f)
                moveTo(12f, 14.2f); lineTo(9.2f, 19.6f)
                moveTo(12f, 14.2f); lineTo(14.8f, 19.6f)
            }
        }
    }

    /** Custom exercise — a pencil (user-made): filled diagonal body with a cut tip. */
    val Custom: ImageVector by lazy {
        icon("ExCustom") {
            fillPath {
                moveTo(8.1f, 18.8f)
                lineTo(5.2f, 15.9f)
                lineTo(15.9f, 5.2f)
                lineTo(18.8f, 8.1f)
                close()
            }
            fillPath {
                moveTo(4.2f, 19.8f)
                lineTo(5.2f, 15.9f)
                lineTo(8.1f, 18.8f)
                close()
            }
        }
    }

    /**
     * The glyph for a movement's equipment list. Collapses stations into classes by priority — the
     * load-defining implement wins (a dumbbell press on a bench is a dumbbell move, a barbell squat
     * in a rack is a barbell move). Empty list = no equipment = bodyweight.
     */
    fun forEquipment(equipment: List<Equipment>): ImageVector {
        val set = equipment.toSet()
        return when {
            Equipment.BARBELL in set || Equipment.EZ_BAR in set || Equipment.TRAP_BAR in set ||
                Equipment.SMITH_MACHINE in set || Equipment.SQUAT_RACK in set -> Barbell
            Equipment.DUMBBELLS in set -> Dumbbell
            Equipment.KETTLEBELL in set -> Kettlebell
            Equipment.CABLE in set -> Cable
            Equipment.MACHINE in set -> Machine
            Equipment.RESISTANCE_BAND in set -> Band
            Equipment.PULL_UP_BAR in set -> PullUpBar
            Equipment.BENCH in set || Equipment.INCLINE_BENCH in set -> Bench
            else -> Bodyweight
        }
    }
}

// Vector-builder plumbing (icon/fillPath/strokePath/circle/roundRect) lives in VectorBuilders.kt,
// shared with the other icon families; the glyphs below stay local so this family evolves on its own.
