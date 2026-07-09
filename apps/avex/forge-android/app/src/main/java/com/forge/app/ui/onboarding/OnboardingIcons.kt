package com.forge.app.ui.onboarding

import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.vector.ImageVector
import com.forge.app.program.Equipment
import com.forge.app.ui.common.circle
import com.forge.app.ui.common.fillPath
import com.forge.app.ui.common.icon
import com.forge.app.ui.common.roundRect
import com.forge.app.ui.common.strokePath

/**
 * Wayfinding glyphs for the onboarding equipment / preset / goal tiles — the SAME matched family and
 * single visual weight as [com.forge.app.ui.nav.NavIcons] and [com.forge.app.ui.settings.SettingsIcons]
 * (24dp viewport, filled silhouettes + the odd stroked line), rendered muted via `Icon(tint = ...)`.
 * Every [Equipment] value has a glyph — [forEquipment] is exhaustive so a new enum entry fails loudly
 * here instead of silently rendering a blank tile.
 */
object OnboardingIcons {

    /** Dumbbell — two chunky rounded weights joined by a solid bar (mirrors SettingsIcons.Program). */
    val Dumbbell: ImageVector by lazy {
        icon("OnbDumbbell") {
            fillPath {
                roundRect(3.6f, 6.6f, 6.6f, 17.4f, 1.3f)
                roundRect(6.6f, 10.6f, 17.4f, 13.4f, 1.2f)
                roundRect(17.4f, 6.6f, 20.4f, 17.4f, 1.3f)
            }
        }
    }

    /** Barbell — a long thin bar with a big plate + collar on each side. */
    val Barbell: ImageVector by lazy {
        icon("OnbBarbell") {
            fillPath {
                roundRect(2.2f, 11.0f, 21.8f, 13.0f, 1.0f)   // bar
                roundRect(4.8f, 6.4f, 7.4f, 17.6f, 1.2f)     // left plate
                roundRect(16.6f, 6.4f, 19.2f, 17.6f, 1.2f)   // right plate
                roundRect(8.2f, 9.0f, 9.6f, 15.0f, 0.7f)     // left collar
                roundRect(14.4f, 9.0f, 15.8f, 15.0f, 0.7f)   // right collar
            }
        }
    }

    /** Squat / power rack — two uprights, a top crossmember and inward J-hooks. */
    val SquatRack: ImageVector by lazy {
        icon("OnbRack") {
            fillPath {
                roundRect(4.8f, 3.8f, 6.8f, 20.2f, 1.0f)     // left upright
                roundRect(17.2f, 3.8f, 19.2f, 20.2f, 1.0f)   // right upright
                roundRect(4.8f, 3.8f, 19.2f, 5.6f, 0.9f)     // top beam
                roundRect(6.8f, 11.0f, 9.4f, 12.8f, 0.6f)    // left J-hook
                roundRect(14.6f, 11.0f, 17.2f, 12.8f, 0.6f)  // right J-hook
            }
        }
    }

    /** Smith machine — a frame with the bar fixed on vertical rails (sliders on each side). */
    val Smith: ImageVector by lazy {
        icon("OnbSmith") {
            fillPath {
                roundRect(4.4f, 3.8f, 6.2f, 20.2f, 0.9f)     // left rail
                roundRect(17.8f, 3.8f, 19.6f, 20.2f, 0.9f)   // right rail
                roundRect(4.4f, 3.8f, 19.6f, 5.6f, 0.9f)     // top beam
                roundRect(6.2f, 11.9f, 17.8f, 13.7f, 0.9f)   // fixed bar
                roundRect(5.6f, 10.4f, 8.0f, 15.2f, 0.9f)    // left slider
                roundRect(16.0f, 10.4f, 18.4f, 15.2f, 0.9f)  // right slider
            }
        }
    }

    /** Trap / hex bar — the hexagonal frame with loading sleeves out each side. */
    val TrapBar: ImageVector by lazy {
        icon("OnbTrapBar") {
            strokePath(1.8f) {
                moveTo(12f, 6.2f); lineTo(17.2f, 9.1f); lineTo(17.2f, 14.9f)
                lineTo(12f, 17.8f); lineTo(6.8f, 14.9f); lineTo(6.8f, 9.1f); close()
                moveTo(2.8f, 12f); lineTo(6.8f, 12f)      // left sleeve
                moveTo(17.2f, 12f); lineTo(21.2f, 12f)    // right sleeve
                moveTo(9.8f, 10.6f); lineTo(9.8f, 13.4f)  // left handle
                moveTo(14.2f, 10.6f); lineTo(14.2f, 13.4f) // right handle
            }
        }
    }

    /** EZ-bar — the zig-zag curl bar with a small weight on each end. */
    val EzBar: ImageVector by lazy {
        icon("OnbEzBar") {
            strokePath(1.9f) {
                moveTo(4.2f, 12f); lineTo(7.0f, 12f)
                lineTo(9.8f, 9.8f); lineTo(14.2f, 14.2f); lineTo(17.0f, 12f)
                lineTo(19.8f, 12f)
            }
            fillPath {
                roundRect(2.2f, 9.2f, 4.2f, 14.8f, 0.9f)
                roundRect(19.8f, 9.2f, 21.8f, 14.8f, 0.9f)
            }
        }
    }

    /** Kettlebell — a filled bell body with a stroked handle arc. */
    val Kettlebell: ImageVector by lazy {
        icon("OnbKettlebell") {
            strokePath(1.9f) {
                moveTo(8.8f, 10.6f)
                curveTo(8.8f, 5.4f, 15.2f, 5.4f, 15.2f, 10.6f)
            }
            fillPath { circle(12f, 14.4f, 5.5f) }
        }
    }

    /** Resistance band — a loose S-curve of band between two grip loops. */
    val Band: ImageVector by lazy {
        icon("OnbBand") {
            strokePath(1.8f) {
                circle(5.4f, 16.8f, 1.9f)
                circle(18.6f, 7.2f, 1.9f)
                moveTo(6.8f, 15.5f)
                curveTo(9.8f, 12.6f, 14.2f, 11.4f, 17.2f, 8.5f)
            }
        }
    }

    /** Cable machine — a top pulley wheel, the cable, and a straight handle. */
    val Cable: ImageVector by lazy {
        icon("OnbCable") {
            strokePath(1.7f) { circle(12f, 5.6f, 2.3f) }
            fillPath { circle(12f, 5.6f, 0.9f) }
            strokePath(1.7f) { moveTo(12f, 7.9f); lineTo(12f, 15.0f) }
            fillPath { roundRect(8.2f, 15.0f, 15.8f, 16.9f, 0.9f) }
        }
    }

    /** Pull-up bar — a doorway: two tall uprights with the bar spanning the top third. */
    val PullUpBar: ImageVector by lazy {
        icon("OnbPullUp") {
            fillPath {
                roundRect(3.8f, 3.8f, 5.8f, 20.2f, 1.0f)
                roundRect(18.2f, 3.8f, 20.2f, 20.2f, 1.0f)
                roundRect(5.8f, 7.2f, 18.2f, 9.0f, 0.9f)
            }
        }
    }

    /** Flat bench — side view: a thick pad line on two legs. */
    val Bench: ImageVector by lazy {
        icon("OnbBench") {
            strokePath(3.0f) { moveTo(4.2f, 10.8f); lineTo(19.8f, 10.8f) }
            strokePath(1.8f) {
                moveTo(7.2f, 12.4f); lineTo(7.2f, 18.2f)
                moveTo(16.8f, 12.4f); lineTo(16.8f, 18.2f)
            }
        }
    }

    /** Incline bench — the same bench with its back pad raised at an angle. */
    val InclineBench: ImageVector by lazy {
        icon("OnbIncline") {
            strokePath(3.0f) {
                moveTo(10.4f, 13.2f); lineTo(19.6f, 13.2f)   // seat
                moveTo(5.4f, 5.4f); lineTo(10.4f, 13.2f)     // raised back
            }
            strokePath(1.8f) {
                moveTo(12.6f, 14.8f); lineTo(12.6f, 18.8f)
                moveTo(17.4f, 14.8f); lineTo(17.4f, 18.8f)
            }
        }
    }

    /** Bodyweight — a jumping-jack figure: filled head, stroked limbs. */
    val Bodyweight: ImageVector by lazy {
        icon("OnbBodyweight") {
            fillPath { circle(12f, 5.4f, 2.1f) }
            strokePath(2.0f) {
                moveTo(12f, 8.2f); lineTo(12f, 14.0f)          // torso
                moveTo(12f, 9.8f); lineTo(6.6f, 12.8f)         // left arm
                moveTo(12f, 9.8f); lineTo(17.4f, 12.8f)        // right arm
                moveTo(12f, 14.0f); lineTo(8.4f, 19.4f)        // left leg
                moveTo(12f, 14.0f); lineTo(15.6f, 19.4f)       // right leg
            }
        }
    }

    /** Machine — a selectorized weight stack under its top beam and pin cable. */
    val Machine: ImageVector by lazy {
        icon("OnbMachine") {
            strokePath(1.8f) {
                moveTo(7.4f, 4.8f); lineTo(16.6f, 4.8f)
                moveTo(12f, 4.8f); lineTo(12f, 10.2f)
            }
            fillPath {
                roundRect(7.4f, 10.2f, 16.6f, 12.6f, 0.8f)
                roundRect(7.4f, 13.4f, 16.6f, 15.8f, 0.8f)
                roundRect(7.4f, 16.6f, 16.6f, 19.0f, 0.8f)
            }
        }
    }

    /** House — the home-gym presets. */
    val House: ImageVector by lazy {
        icon("OnbHouse") {
            strokePath(1.8f) {
                moveTo(3.8f, 11.8f); lineTo(12f, 4.4f); lineTo(20.2f, 11.8f)
                moveTo(5.8f, 10.4f); lineTo(5.8f, 19.4f); lineTo(18.2f, 19.4f); lineTo(18.2f, 10.4f)
            }
            fillPath { roundRect(10.3f, 14.2f, 13.7f, 19.4f, 0.6f) }
        }
    }

    /** Building — the commercial full-gym preset: a stroked block with lit windows. */
    val Building: ImageVector by lazy {
        icon("OnbBuilding") {
            strokePath(1.7f) { roundRect(5.0f, 4.2f, 19.0f, 19.8f, 1.2f) }
            fillPath {
                roundRect(7.7f, 7.0f, 9.7f, 9.0f, 0.4f); roundRect(11.0f, 7.0f, 13.0f, 9.0f, 0.4f); roundRect(14.3f, 7.0f, 16.3f, 9.0f, 0.4f)
                roundRect(7.7f, 10.8f, 9.7f, 12.8f, 0.4f); roundRect(11.0f, 10.8f, 13.0f, 12.8f, 0.4f); roundRect(14.3f, 10.8f, 16.3f, 12.8f, 0.4f)
                roundRect(10.6f, 15.4f, 13.4f, 19.8f, 0.5f)  // door
            }
        }
    }

    // ── Goal glyphs ───────────────────────────────────────────────────────────

    /** Build muscle — a flexed arm: thick L-shaped limb with a bicep bump. */
    val Muscle: ImageVector by lazy {
        icon("OnbMuscle") {
            strokePath(4.0f) {
                moveTo(5.2f, 15.6f); lineTo(12.4f, 15.6f)     // upper arm
                moveTo(12.4f, 15.6f); lineTo(16.6f, 7.6f)     // forearm up to the fist (round cap)
            }
            fillPath { circle(9.4f, 12.0f, 2.7f) }            // bicep
        }
    }

    /** Lose weight — a flame with a hollow core. */
    val Flame: ImageVector by lazy {
        icon("OnbFlame") {
            fillPath(PathFillType.EvenOdd) {
                moveTo(12f, 3.6f)
                curveTo(13.2f, 6.6f, 17.0f, 8.4f, 17.0f, 12.6f)
                curveTo(17.0f, 16.6f, 14.7f, 19.4f, 12f, 19.4f)
                curveTo(9.3f, 19.4f, 7.0f, 16.6f, 7.0f, 12.6f)
                curveTo(7.0f, 10.3f, 8.1f, 8.6f, 9.4f, 7.2f)
                curveTo(9.9f, 8.9f, 10.9f, 9.8f, 12f, 9.8f)
                curveTo(12.8f, 9.8f, 12.4f, 6.6f, 12f, 3.6f)
                close()
                circle(12f, 14.8f, 2.2f)
            }
        }
    }

    /** General fitness — a heartbeat trace. */
    val Pulse: ImageVector by lazy {
        icon("OnbPulse") {
            strokePath(1.9f) {
                moveTo(3.0f, 12.6f); lineTo(8.0f, 12.6f); lineTo(10.0f, 7.4f)
                lineTo(13.4f, 17.2f); lineTo(15.4f, 12.6f); lineTo(21.0f, 12.6f)
            }
        }
    }

    /** Exhaustive equipment → glyph mapping. */
    fun forEquipment(e: Equipment): ImageVector = when (e) {
        Equipment.DUMBBELLS -> Dumbbell
        Equipment.BARBELL -> Barbell
        Equipment.SQUAT_RACK -> SquatRack
        Equipment.SMITH_MACHINE -> Smith
        Equipment.TRAP_BAR -> TrapBar
        Equipment.EZ_BAR -> EzBar
        Equipment.KETTLEBELL -> Kettlebell
        Equipment.RESISTANCE_BAND -> Band
        Equipment.CABLE -> Cable
        Equipment.PULL_UP_BAR -> PullUpBar
        Equipment.BENCH -> Bench
        Equipment.INCLINE_BENCH -> InclineBench
        Equipment.BODYWEIGHT_ONLY -> Bodyweight
        Equipment.MACHINE -> Machine
    }

    /** Preset id → glyph; unknown ids (future presets) fall back to the building. */
    fun forPreset(id: String): ImageVector = when (id) {
        "developer" -> House
        "commercial" -> Building
        "basic-gym" -> SquatRack
        "home-barbell" -> Barbell
        "db-bench" -> Bench
        "dumbbells" -> Dumbbell
        "bodyweight" -> Bodyweight
        else -> Building
    }

    /** Goal key → glyph. */
    fun forGoal(key: String): ImageVector = when (key) {
        "build_muscle" -> Muscle
        "get_stronger" -> Barbell
        "lose_weight" -> Flame
        else -> Pulse
    }
}

// Vector-builder plumbing (icon/fillPath/strokePath/circle/roundRect) lives in VectorBuilders.kt,
// shared with the other icon families; the glyphs above stay local so this family evolves on its own.
