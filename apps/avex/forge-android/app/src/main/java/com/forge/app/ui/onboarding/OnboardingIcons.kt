package com.forge.app.ui.onboarding

import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathBuilder
import com.forge.app.program.Equipment
import com.forge.app.ui.common.circle
import com.forge.app.ui.common.fillPath
import com.forge.app.ui.common.icon
import com.forge.app.ui.common.roundRect
import com.forge.app.ui.common.strokePath

/**
 * Wayfinding glyphs for the onboarding equipment / preset / goal tiles — the SAME matched family and
 * single visual weight as [com.forge.app.ui.nav.NavIcons] and [com.forge.app.ui.settings.SettingsIcons]
 * (24dp viewport, one limb thickness across fills and strokes alike), rendered muted via
 * `Icon(tint = ...)`.
 * Every [Equipment] value has a glyph — [forEquipment] is exhaustive so a new enum entry fails loudly
 * here instead of silently rendering a blank tile.
 */
object OnboardingIcons {

    /**
     * **One limb thickness for the whole family.** Redrawn 2026-08-22: the glyphs had grown from two
     * incompatible constructions — heavy filled silhouettes (dumbbell, rack, Smith, dip tower) beside
     * 1.7dp stroked outlines (trap bar, band, cable, bench, house) — so at 24dp muted on near-black
     * half the equipment grid read as solid blocks and half as faint wireframe. Optical weight, not
     * shape, was what made the grid look messy.
     *
     * Every glyph draws its lines at [LIMB] and fills its box out to roughly x 2..22 / y 4..20, so no
     * tile holds a glyph half the size or twice the weight of its neighbour's. Filled shapes are
     * MASSES — a plate, a pad, a weight stack — and may be thicker than a line; what is not allowed
     * is a line drawn at one weight here and another there. Detail that disappears at 24dp was cut
     * rather than shrunk: the Smith's sliders, the trap bar's handles, two rows of windows.
     *
     * [LIMB] is 1.8 because that is what `NavIcons` and `SettingsIcons` draw at, and this family is
     * required to match them. A first pass set it to 2.2 and made every onboarding tile visibly
     * heavier than the same glyph weight everywhere else in the app.
     */
    private const val LIMB = 1.8f

    /** The lines that carry a body rather than an edge — a torso, an arm, a strap, a handle. */
    private const val LIMB_BOLD = 2.2f

    /** Pads, grips, tyres: a mass drawn as a thick stroke so it keeps round ends at any angle. */
    private const val MASS_THIN = 3.2f

    /** Dumbbell heads — the heaviest mass in the family. */
    private const val MASS = 4.6f

    /** A mass drawn as a stroke — a stadium from ([x1],[y1]) to ([x2],[y2]), [w] thick. Heads, pads
     *  and grips are built this way so a rotated mass keeps its rounded ends. */
    private fun PathBuilder.seg(x1: Float, y1: Float, x2: Float, y2: Float) {
        moveTo(x1, y1); lineTo(x2, y2)
    }

    /** Dumbbell — tilted 45°, two fat heads on a short handle. The tilt is the whole trick: level,
     *  a dumbbell at 24dp is the letter H, and it sits too close to the [Barbell] beside it. */
    val Dumbbell: ImageVector by lazy {
        icon("OnbDumbbell") {
            strokePath(LIMB_BOLD) { seg(9.2f, 14.8f, 14.8f, 9.2f) }                     // handle
            strokePath(MASS) {
                seg(5.5f, 14.3f, 9.7f, 18.5f)                                           // lower head
                seg(14.3f, 5.5f, 18.5f, 9.7f)                                           // upper head
            }
        }
    }

    /** Barbell — a full-width bar loaded with a big and a small plate each side, and a collar. */
    val Barbell: ImageVector by lazy {
        icon("OnbBarbell") {
            strokePath(LIMB) { seg(1.6f, 12f, 22.4f, 12f) }
            fillPath {
                roundRect(6.2f, 5.0f, 8.6f, 19.0f, 1.2f)     // big plates
                roundRect(15.4f, 5.0f, 17.8f, 19.0f, 1.2f)
                roundRect(3.6f, 7.8f, 5.6f, 16.2f, 1.0f)     // small plates
                roundRect(18.4f, 7.8f, 20.4f, 16.2f, 1.0f)
                roundRect(9.0f, 10.2f, 10.2f, 13.8f, 0.5f)   // collars
                roundRect(13.8f, 10.2f, 15.0f, 13.8f, 0.5f)
            }
        }
    }

    /** Squat rack — two uprights on their feet with a loaded bar sitting in the hooks. The plates
     *  outside the uprights are what make it a rack you squat in rather than a doorway. */
    val SquatRack: ImageVector by lazy {
        icon("OnbRack") {
            strokePath(LIMB) { seg(1.6f, 8.6f, 22.4f, 8.6f) }                           // racked bar
            fillPath {
                roundRect(6.0f, 3.2f, 8.0f, 20.4f, 1.0f)     // uprights
                roundRect(16.0f, 3.2f, 18.0f, 20.4f, 1.0f)
                roundRect(3.6f, 19.2f, 10.4f, 21.0f, 0.9f)   // feet
                roundRect(13.6f, 19.2f, 20.4f, 21.0f, 0.9f)
                roundRect(2.2f, 4.6f, 4.4f, 12.6f, 1.0f)     // plates
                roundRect(19.6f, 4.6f, 21.8f, 12.6f, 1.0f)
            }
        }
    }

    /** Smith machine — a closed frame with the bar locked to the rails by two carriages. Closed
     *  top and carriages say Smith; feet and loaded plates say [SquatRack]. */
    val Smith: ImageVector by lazy {
        icon("OnbSmith") {
            strokePath(LIMB) { seg(1.8f, 12.2f, 22.2f, 12.2f) }                         // the bar
            fillPath {
                roundRect(3.4f, 2.8f, 20.6f, 4.8f, 1.0f)     // top beam
                roundRect(3.4f, 19.2f, 20.6f, 21.2f, 1.0f)   // base
                roundRect(5.6f, 4.8f, 7.4f, 19.2f, 0.4f)     // rails
                roundRect(16.6f, 4.8f, 18.4f, 19.2f, 0.4f)
                roundRect(4.4f, 10.2f, 8.6f, 14.2f, 1.0f)    // carriages
                roundRect(15.4f, 10.2f, 19.6f, 14.2f, 1.0f)
            }
        }
    }

    /** Trap / hex bar — from above: the hexagon with its two handles inside and a sleeve out each
     *  side. No plates: they turned the hexagon into a coin at 24dp. */
    val TrapBar: ImageVector by lazy {
        icon("OnbTrapBar") {
            strokePath(LIMB) {
                moveTo(12f, 5.4f); lineTo(17.6f, 8.7f); lineTo(17.6f, 15.3f)
                lineTo(12f, 18.6f); lineTo(6.4f, 15.3f); lineTo(6.4f, 8.7f); close()
                seg(1.8f, 12f, 6.4f, 12f)
                seg(17.6f, 12f, 22.2f, 12f)
            }
            strokePath(LIMB) {
                seg(9.6f, 10.0f, 9.6f, 14.0f)                // handles
                seg(14.4f, 10.0f, 14.4f, 14.0f)
            }
            fillPath {
                roundRect(1.8f, 9.8f, 3.4f, 14.2f, 0.8f)     // sleeve ends
                roundRect(20.6f, 9.8f, 22.2f, 14.2f, 0.8f)
            }
        }
    }

    /** EZ-bar — the barbell's plates on a bar with the curl bend in the middle. */
    val EzBar: ImageVector by lazy {
        icon("OnbEzBar") {
            strokePath(LIMB) {
                moveTo(1.6f, 12f); lineTo(7.4f, 12f)
                lineTo(9.4f, 15.6f); lineTo(12f, 8.4f); lineTo(14.6f, 15.6f)
                lineTo(16.6f, 12f); lineTo(22.4f, 12f)
            }
            fillPath {
                roundRect(3.6f, 6.8f, 5.8f, 17.2f, 1.1f)
                roundRect(18.2f, 6.8f, 20.4f, 17.2f, 1.1f)
            }
        }
    }

    /** Kettlebell — a flat-bottomed bell under a thick arched handle. */
    val Kettlebell: ImageVector by lazy {
        icon("OnbKettlebell") {
            strokePath(LIMB_BOLD) {
                moveTo(8.6f, 11.2f)
                curveTo(7.6f, 2.8f, 16.4f, 2.8f, 15.4f, 11.2f)
            }
            fillPath {
                moveTo(7.8f, 20.6f); lineTo(16.2f, 20.6f)
                arcTo(6.4f, 6.4f, 0f, true, false, 7.8f, 20.6f)
                close()
            }
        }
    }

    /** Resistance band — a tube hanging in a loop between two D-handles. The stirrups are the
     *  read: with plain grips the loop was a horseshoe magnet. */
    val Band: ImageVector by lazy {
        icon("OnbBand") {
            fillPath {
                roundRect(2.2f, 2.6f, 7.4f, 4.6f, 1.0f)      // grips
                roundRect(16.6f, 2.6f, 21.8f, 4.6f, 1.0f)
            }
            strokePath(LIMB) {
                moveTo(2.8f, 4.4f); lineTo(4.8f, 8.2f); lineTo(6.8f, 4.4f)       // stirrups
                moveTo(17.2f, 4.4f); lineTo(19.2f, 8.2f); lineTo(21.2f, 4.4f)
                moveTo(4.8f, 8.2f)
                curveTo(4.8f, 23.0f, 19.2f, 23.0f, 19.2f, 8.2f)                  // the tube
            }
        }
    }

    /** Cable machine — a column with its arm out to a pulley, the cable dropping to a stirrup
     *  handle. The pulley and the hanging handle are the read; the weight stack is [Machine]'s. */
    val Cable: ImageVector by lazy {
        icon("OnbCable") {
            fillPath {
                roundRect(3.2f, 2.8f, 5.6f, 21.0f, 1.0f)     // column
                roundRect(2.0f, 19.2f, 10.8f, 21.0f, 0.9f)   // foot
                roundRect(3.2f, 2.8f, 15.6f, 5.0f, 1.0f)     // arm
                roundRect(14.8f, 18.2f, 22.0f, 20.4f, 1.1f)  // handle grip
            }
            strokePath(LIMB) {
                circle(17.4f, 5.4f, 2.0f)                    // pulley
                seg(18.4f, 7.2f, 18.4f, 13.8f)               // cable
                moveTo(16.0f, 18.4f); lineTo(18.4f, 13.8f); lineTo(20.8f, 18.4f)  // stirrup
            }
        }
    }

    /** Pull-up bar — somebody hanging from it. The hang is the read: a beam on posts is already
     *  [SquatRack] and [Smith]. Arms up into a bar, where [DipStation]'s push down onto posts. */
    val PullUpBar: ImageVector by lazy {
        icon("OnbPullUp") {
            fillPath { circle(12f, 9.0f, 2.2f) }            // head
            strokePath(LIMB_BOLD) {
                seg(2.0f, 3.4f, 22.0f, 3.4f)                 // the bar
                moveTo(6.8f, 3.4f); lineTo(9.8f, 12.4f); lineTo(14.2f, 12.4f); lineTo(17.2f, 3.4f)  // arms
                seg(12f, 12.4f, 12f, 16.4f)                  // torso
                moveTo(10.2f, 21.0f); lineTo(12f, 16.4f); lineTo(13.8f, 21.0f)  // legs
            }
        }
    }

    /** Flat bench — side view: one thick pad, two posts, a foot under each. */
    val Bench: ImageVector by lazy {
        icon("OnbBench") {
            strokePath(MASS_THIN) {
                seg(4.2f, 9.6f, 19.8f, 9.6f)                 // pad
                seg(4.4f, 19.8f, 9.8f, 19.8f)                // feet
                seg(14.2f, 19.8f, 19.6f, 19.8f)
            }
            fillPath {
                roundRect(6.2f, 10.8f, 8.0f, 19.0f, 0.6f)    // posts
                roundRect(16.0f, 10.8f, 17.8f, 19.0f, 0.6f)
            }
        }
    }

    /** Incline bench — the same pad and feet with the back raised. The pad weight is shared with
     *  [Bench] so the two read as one bench in two positions. */
    val InclineBench: ImageVector by lazy {
        icon("OnbIncline") {
            strokePath(MASS_THIN) {
                seg(11.2f, 13.0f, 19.8f, 13.0f)              // seat
                seg(10.0f, 12.2f, 5.2f, 4.2f)                // raised back
                seg(8.2f, 19.8f, 20.0f, 19.8f)               // base
            }
            strokePath(LIMB) { seg(7.6f, 8.2f, 10.2f, 19.8f) }                          // back strut
            fillPath {
                roundRect(12.4f, 14.2f, 14.2f, 19.0f, 0.6f)  // posts
                roundRect(16.6f, 14.2f, 18.4f, 19.0f, 0.6f)
            }
        }
    }

    /** Dip station — somebody held up on straight arms between two posts. Arms pushing DOWN onto
     *  the posts, where [PullUpBar]'s reach up into a bar. */
    val DipStation: ImageVector by lazy {
        icon("OnbDip") {
            fillPath {
                circle(12f, 4.4f, 2.2f)                      // head
                roundRect(3.0f, 11.4f, 5.4f, 21.0f, 1.0f)    // posts
                roundRect(18.6f, 11.4f, 21.0f, 21.0f, 1.0f)
            }
            strokePath(LIMB_BOLD) {
                moveTo(4.2f, 11.6f); lineTo(8.4f, 7.6f); lineTo(15.6f, 7.6f); lineTo(19.8f, 11.6f)  // arms
                seg(12f, 7.6f, 12f, 13.6f)                   // torso
                moveTo(10.4f, 18.0f); lineTo(12f, 13.6f); lineTo(13.6f, 18.0f)  // legs, tucked
            }
        }
    }

    /** Suspension trainer — anchor ring, two straps, a stirrup handle on each. */
    val Suspension: ImageVector by lazy {
        icon("OnbSuspension") {
            strokePath(LIMB) {
                circle(12f, 3.4f, 1.6f)                      // anchor
                moveTo(4.6f, 18.8f); lineTo(6.8f, 14.6f); lineTo(9.0f, 18.8f)   // left stirrup
                moveTo(15.0f, 18.8f); lineTo(17.2f, 14.6f); lineTo(19.4f, 18.8f) // right stirrup
            }
            strokePath(LIMB_BOLD) {
                seg(11.0f, 5.0f, 6.8f, 14.6f)                // straps
                seg(13.0f, 5.0f, 17.2f, 14.6f)
            }
            fillPath {
                roundRect(3.4f, 18.4f, 10.2f, 20.6f, 1.1f)   // grips
                roundRect(13.8f, 18.4f, 20.6f, 20.6f, 1.1f)
            }
        }
    }

    /** Ab wheel — side view: a thick tyre and hub, a grip out each side of the axle. */
    val AbWheel: ImageVector by lazy {
        icon("OnbAbWheel") {
            strokePath(MASS_THIN) { circle(12f, 12f, 5.8f) }
            fillPath { circle(12f, 12f, 1.8f) }
            strokePath(MASS_THIN) {
                seg(2.6f, 12f, 4.4f, 12f)
                seg(19.6f, 12f, 21.4f, 12f)
            }
        }
    }

    /** Bodyweight — a push-up, the one movement that needs nothing at all. Side-on and low, so it
     *  never reads as the upright figures on [PullUpBar] and [DipStation]. */
    val Bodyweight: ImageVector by lazy {
        icon("OnbBodyweight") {
            fillPath { circle(19.4f, 8.2f, 2.2f) }          // head
            strokePath(LIMB_BOLD) {
                seg(16.6f, 10.4f, 2.8f, 17.8f)               // body, one straight line
                seg(16.6f, 10.4f, 16.6f, 17.8f)              // arm
            }
            strokePath(LIMB) { seg(1.8f, 20.2f, 22.2f, 20.2f) }                         // floor
        }
    }

    /** Machine — a selectorized stack: top cap, guide rods, four plates and the pin. */
    val Machine: ImageVector by lazy {
        icon("OnbMachine") {
            strokePath(LIMB) {
                seg(7.8f, 4.8f, 7.8f, 20.4f)                 // guide rods
                seg(16.2f, 4.8f, 16.2f, 20.4f)
                seg(12f, 4.8f, 12f, 8.4f)                    // lift rod
            }
            strokePath(2.0f) { seg(18.2f, 13.0f, 20.6f, 13.0f) }                        // pin
            fillPath {
                roundRect(4.4f, 2.6f, 19.6f, 4.8f, 1.0f)     // top cap
                roundRect(5.4f, 8.4f, 18.6f, 10.8f, 0.8f)    // plates
                roundRect(5.4f, 11.8f, 18.6f, 14.2f, 0.8f)
                roundRect(5.4f, 15.2f, 18.6f, 17.6f, 0.8f)
                roundRect(5.4f, 18.6f, 18.6f, 21.0f, 0.8f)
                circle(21.0f, 13.0f, 1.3f)                   // pin knob
            }
        }
    }

    // ── Preset glyphs ─────────────────────────────────────────────────────────
    // Presets are PLACES, so they get their own glyphs rather than borrowing a piece of gear: a
    // commercial building for the full gym, a house with the defining piece inside for the three
    // home setups. The single-kit presets (dumbbells, bands, bodyweight) keep their kit's glyph.

    /** Everything gym — a commercial building: flat roof, a dumbbell sign, a wide entrance. */
    val Building: ImageVector by lazy {
        icon("OnbBuilding") {
            strokePath(LIMB) {
                moveTo(4.6f, 5.6f); lineTo(4.6f, 20.4f); lineTo(19.4f, 20.4f); lineTo(19.4f, 5.6f)
            }
            strokePath(1.4f) { seg(9.8f, 10.0f, 14.2f, 10.0f) }                         // sign bar
            fillPath {
                roundRect(2.6f, 3.2f, 21.4f, 5.8f, 1.0f)     // roof
                roundRect(7.8f, 8.0f, 9.8f, 12.0f, 0.8f)     // sign weights
                roundRect(14.2f, 8.0f, 16.2f, 12.0f, 0.8f)
                roundRect(8.4f, 14.4f, 15.6f, 20.4f, 0.8f)   // entrance
            }
        }
    }

    private fun ImageVector.Builder.house() = strokePath(LIMB) {
        moveTo(2.6f, 11.2f); lineTo(12f, 3.4f); lineTo(21.4f, 11.2f)
        moveTo(5.0f, 9.2f); lineTo(5.0f, 20.6f); lineTo(19.0f, 20.6f); lineTo(19.0f, 9.2f)
    }

    /** Home gym, big — a house with a loaded barbell inside. */
    val HouseBarbell: ImageVector by lazy {
        icon("OnbHouseBarbell") {
            house()
            strokePath(1.4f) { seg(7.0f, 15.6f, 17.0f, 15.6f) }
            fillPath {
                roundRect(8.4f, 12.2f, 10.2f, 19.0f, 0.8f)
                roundRect(13.8f, 12.2f, 15.6f, 19.0f, 0.8f)
            }
        }
    }

    /** Home gym, small — a house with a dumbbell inside, tilted like [Dumbbell]. */
    val HouseDumbbell: ImageVector by lazy {
        icon("OnbHouseDumbbell") {
            house()
            strokePath(1.8f) { seg(10.6f, 16.8f, 13.4f, 14.0f) }
            strokePath(3.0f) {
                seg(8.6f, 16.2f, 10.8f, 18.4f)
                seg(13.2f, 11.6f, 15.4f, 13.8f)
            }
        }
    }

    /** Home machine gym — a house with a weight stack inside. */
    val HouseMachine: ImageVector by lazy {
        icon("OnbHouseMachine") {
            house()
            fillPath {
                roundRect(8.4f, 11.6f, 15.6f, 13.4f, 0.6f)
                roundRect(8.4f, 14.2f, 15.6f, 16.0f, 0.6f)
                roundRect(8.4f, 16.8f, 15.6f, 18.6f, 0.6f)
            }
        }
    }

    // ── Goal glyphs ───────────────────────────────────────────────────────────

    /** Build muscle — a flexed arm: upper arm, forearm, and the bicep between them. */
    val Muscle: ImageVector by lazy {
        icon("OnbMuscle") {
            // The arm is a MASS, so it is drawn at a mass's weight, not at [LIMB].
            strokePath(4.4f) {
                moveTo(5.2f, 17.8f); lineTo(12.6f, 17.8f)      // upper arm
                moveTo(12.6f, 17.8f); lineTo(17.0f, 9.8f)      // forearm
            }
            fillPath {
                circle(17.4f, 8.4f, 2.9f)                      // fist
                circle(9.8f, 14.2f, 3.9f)                      // bicep, clear above the arm
            }
        }
    }

    /** Lose weight — a flame with a hollow core. */
    val Flame: ImageVector by lazy {
        icon("OnbFlame") {
            fillPath(PathFillType.EvenOdd) {
                moveTo(12f, 2.8f)
                curveTo(13.6f, 6.4f, 18.6f, 8.2f, 18.6f, 12.8f)
                curveTo(18.6f, 17.4f, 15.6f, 20.6f, 12f, 20.6f)
                curveTo(8.4f, 20.6f, 5.4f, 17.4f, 5.4f, 12.8f)
                curveTo(5.4f, 10.0f, 6.8f, 7.9f, 8.6f, 6.2f)
                curveTo(9.2f, 8.4f, 10.6f, 9.6f, 12f, 9.6f)
                curveTo(13.1f, 9.6f, 12.5f, 6.4f, 12f, 2.8f)
                close()
                circle(12f, 15.4f, 2.8f)
            }
        }
    }

    /** General fitness — a heartbeat trace. */
    val Pulse: ImageVector by lazy {
        icon("OnbPulse") {
            strokePath(LIMB) {
                moveTo(2.4f, 12.6f); lineTo(8.0f, 12.6f); lineTo(10.0f, 6.8f)
                lineTo(13.6f, 17.6f); lineTo(15.6f, 12.6f); lineTo(21.6f, 12.6f)
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
        Equipment.DIP_STATION -> DipStation
        Equipment.SUSPENSION -> Suspension
        Equipment.AB_WHEEL -> AbWheel
        Equipment.BODYWEIGHT_ONLY -> Bodyweight
        Equipment.MACHINE -> Machine
    }

    /** Preset id → glyph; unknown ids (future presets) fall back to the building. */
    fun forPreset(id: String): ImageVector = when (id) {
        "everything" -> Building
        "basic-gym" -> Machine
        "home-big" -> HouseBarbell
        "home-small" -> HouseDumbbell
        "developer" -> HouseMachine
        "dumbbells" -> Dumbbell
        "bands-bw" -> Band
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
