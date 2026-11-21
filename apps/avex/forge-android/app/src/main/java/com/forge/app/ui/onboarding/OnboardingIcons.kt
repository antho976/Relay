package com.forge.app.ui.onboarding

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathBuilder
import androidx.compose.ui.graphics.vector.path
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

    /** Barbell — side on: a big and a small plate each side, butted together, the sleeve showing
     *  past them and the bar bare between. Four masses and a line; the collars and the gaps between
     *  plates were cut because at 26dp they turned the bar into a comb. */
    val Barbell: ImageVector by lazy {
        icon("OnbBarbell") {
            strokePath(LIMB) {
                seg(1.4f, 12f, 3.0f, 12f)                    // sleeves
                seg(21.0f, 12f, 22.6f, 12f)
                seg(8.8f, 12f, 15.2f, 12f)                   // the bar
            }
            fillPath {
                roundRect(5.6f, 4.4f, 8.8f, 19.6f, 1.4f)     // big plates
                roundRect(15.2f, 4.4f, 18.4f, 19.6f, 1.4f)
                roundRect(3.0f, 7.6f, 5.6f, 16.4f, 1.1f)     // small plates
                roundRect(18.4f, 7.6f, 21.0f, 16.4f, 1.1f)
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

    /** Cable machine — the pulley itself: a wheel, the stack hanging off one side of the cable and
     *  the stirrup handle off the other. The whole mechanism in one glyph; a column with an arm out
     *  read as a gallows. */
    val Cable: ImageVector by lazy {
        icon("OnbCable") {
            strokePath(LIMB) {
                circle(12f, 5.8f, 3.6f)                      // pulley wheel
                seg(8.4f, 5.8f, 8.4f, 12.0f)                 // cable to the stack
                seg(15.6f, 5.8f, 15.6f, 12.6f)               // cable to the handle
                moveTo(15.6f, 12.6f); lineTo(13.0f, 17.8f)   // stirrup
                moveTo(15.6f, 12.6f); lineTo(18.2f, 17.8f)
            }
            fillPath {
                circle(12f, 5.8f, 1.2f)                      // hub
                roundRect(4.8f, 12.0f, 12.0f, 15.2f, 1.0f)   // stack
                roundRect(4.8f, 16.0f, 12.0f, 19.2f, 1.0f)
            }
            strokePath(MASS_THIN) { seg(12.8f, 19.0f, 18.4f, 19.0f) }                   // handle grip
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

    /** Flat bench — side on, with the bar racked in its uprights at the head end, seen end-on as a
     *  plate. A bare bench in profile is a table; the racked plate is what makes it a bench you
     *  press on, and keeps it clear of [InclineBench]'s raised back. */
    val Bench: ImageVector by lazy {
        icon("OnbBench") {
            strokePath(3.4f) { seg(2.8f, 13.2f, 15.6f, 13.2f) }                         // pad
            strokePath(LIMB_BOLD) {
                seg(5.2f, 15.0f, 5.2f, 20.4f)                // legs
                seg(13.2f, 15.0f, 13.2f, 20.4f)
                seg(19.2f, 9.6f, 19.2f, 20.4f)               // upright
                seg(2.8f, 20.4f, 21.6f, 20.4f)               // floor rail
            }
            fillPath(PathFillType.EvenOdd) {
                circle(19.2f, 6.4f, 3.8f)                    // racked plate, end-on
                circle(19.2f, 6.4f, 1.1f)
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

    /** Machine — a seated station: the stack tower with its lift rod on one side, the seat and its
     *  back on the other. The seat is the read; a stack alone looked like a bookshelf, and the
     *  hanging stack is already [Cable]'s. */
    val Machine: ImageVector by lazy {
        icon("OnbMachine") {
            strokePath(LIMB) {
                moveTo(3.0f, 20.8f); lineTo(3.0f, 3.0f); lineTo(11.0f, 3.0f); lineTo(11.0f, 20.8f)  // tower
                seg(7.0f, 3.0f, 7.0f, 8.8f)                  // lift rod
                seg(1.8f, 20.8f, 22.2f, 20.8f)               // base
                seg(17.6f, 15.6f, 17.6f, 20.8f)              // seat post
            }
            fillPath {
                roundRect(4.4f, 8.8f, 9.6f, 11.2f, 0.7f)     // stack
                roundRect(4.4f, 12.0f, 9.6f, 14.4f, 0.7f)
                roundRect(4.4f, 15.2f, 9.6f, 17.6f, 0.7f)
            }
            strokePath(MASS_THIN) {
                moveTo(20.6f, 4.8f); lineTo(19.8f, 14.0f); lineTo(14.4f, 14.0f)          // back + seat
            }
        }
    }

    // ── Preset glyphs ─────────────────────────────────────────────────────────
    // Presets are PLACES, so they get their own glyphs rather than borrowing a piece of gear: a
    // commercial building for the full gym, a roofline over the defining piece for the three home
    // setups. The single-kit presets (dumbbells, bands, bodyweight) keep their kit's glyph.

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

    /** The home roofline: a pitched roof and its chimney, and no walls. Walls boxed the gear into a
     *  third of the tile, so the piece inside shrank to a sticker; under an open roof it is drawn
     *  at full size and the roof alone says "home". */
    private fun ImageVector.Builder.roof() = strokePath(LIMB_BOLD) {
        moveTo(2.6f, 10.0f); lineTo(12f, 3.0f); lineTo(21.4f, 10.0f)
        seg(17.6f, 7.2f, 17.6f, 3.6f)                        // chimney
    }

    /** Home gym, big — a full barbell under the roof. */
    val HouseBarbell: ImageVector by lazy {
        icon("OnbHouseBarbell") {
            roof()
            strokePath(LIMB) {
                seg(1.8f, 16.6f, 2.8f, 16.6f)
                seg(7.6f, 16.6f, 16.4f, 16.6f)
                seg(21.2f, 16.6f, 22.2f, 16.6f)
            }
            fillPath {
                roundRect(4.6f, 12.2f, 7.6f, 21.0f, 1.2f)
                roundRect(16.4f, 12.2f, 19.4f, 21.0f, 1.2f)
                roundRect(2.8f, 14.0f, 4.6f, 19.2f, 0.9f)
                roundRect(19.4f, 14.0f, 21.2f, 19.2f, 0.9f)
            }
        }
    }

    /** Home gym, small — a dumbbell under the roof, level and short so it never reads as the bar. */
    val HouseDumbbell: ImageVector by lazy {
        icon("OnbHouseDumbbell") {
            roof()
            strokePath(LIMB_BOLD) { seg(9.6f, 16.6f, 14.4f, 16.6f) }
            fillPath {
                roundRect(6.4f, 12.4f, 9.6f, 20.8f, 1.4f)
                roundRect(14.4f, 12.4f, 17.6f, 20.8f, 1.4f)
                roundRect(4.6f, 14.2f, 6.4f, 19.0f, 0.9f)
                roundRect(17.6f, 14.2f, 19.4f, 19.0f, 0.9f)
            }
        }
    }

    /** Home machine gym — a weight stack hanging on its rod from the ridge. */
    val HouseMachine: ImageVector by lazy {
        icon("OnbHouseMachine") {
            roof()
            strokePath(LIMB) { seg(12f, 9.0f, 12f, 12.0f) }
            fillPath {
                roundRect(7.0f, 12.0f, 17.0f, 14.6f, 0.9f)
                roundRect(7.0f, 15.4f, 17.0f, 18.0f, 0.9f)
                roundRect(7.0f, 18.8f, 17.0f, 21.4f, 0.9f)
            }
        }
    }

    // ── Goal glyphs ───────────────────────────────────────────────────────────
    // The four goals are one set, drawn as outlines at [LIMB] so none of them outweighs the others in
    // the option list: two were solid silhouettes and one a bare line until 2026-09-26, and the eye
    // ranked the goals by ink. The lifter is the one figure, drawn at [LIMB_BOLD] like the family's
    // other figures ([PullUpBar], [DipStation]).

    /** Build muscle — a flexed arm in the pose everyone knows from the emoji: the forearm up the
     *  left with the fist curled in at the top, the bicep a big round peak on the right, and the
     *  crease where they fold together. Redrawn 2026-09-27: the side-on arm before it (a flat upper
     *  arm with the forearm rising off its end) read as a swan, neck and head. */
    val Muscle: ImageVector by lazy {
        icon("OnbMuscle") {
            strokePath(LIMB) {
                moveTo(12.0f, 13.4f)                                  // bicep, from the crease
                curveTo(13.6f, 10.2f, 19.4f, 9.8f, 21.0f, 13.8f)
                curveTo(22.2f, 16.8f, 20.2f, 21.2f, 14.0f, 21.2f)    // round under to the elbow
                lineTo(6.0f, 21.2f)
                curveTo(4.2f, 21.2f, 2.8f, 20.0f, 2.8f, 18.2f)       // elbow
                curveTo(2.8f, 11.4f, 4.6f, 3.8f, 8.6f, 2.8f)         // back of the forearm
                curveTo(10.8f, 2.2f, 13.2f, 3.0f, 13.2f, 5.0f)       // fist
                curveTo(13.2f, 6.4f, 12.0f, 7.2f, 10.6f, 7.0f)
                curveTo(9.8f, 6.9f, 9.2f, 6.4f, 9.0f, 5.8f)          // curled fingers
                moveTo(10.2f, 7.0f)                                   // inside of the forearm
                curveTo(8.6f, 9.4f, 8.8f, 13.0f, 7.8f, 15.8f)
                moveTo(15.2f, 14.8f)                                  // the fold
                curveTo(12.8f, 13.8f, 9.8f, 14.6f, 7.8f, 16.8f)
            }
        }
    }

    /** Get stronger — a lifter with the bar locked out overhead. Its own glyph rather than the
     *  [Barbell] from the gear grid, so a goal never reads as a piece of equipment. */
    val Lifter: ImageVector by lazy {
        icon("OnbLifter") {
            fillPath {
                circle(12f, 9.0f, 2.1f)                      // head
                roundRect(2.4f, 1.6f, 4.8f, 8.4f, 1.1f)      // plates
                roundRect(19.2f, 1.6f, 21.6f, 8.4f, 1.1f)
            }
            strokePath(LIMB) { seg(1.4f, 5.0f, 22.6f, 5.0f) }                           // the bar
            strokePath(LIMB_BOLD) {
                moveTo(7.4f, 5.0f); lineTo(9.4f, 12.0f); lineTo(14.6f, 12.0f); lineTo(16.6f, 5.0f)  // arms
                seg(12f, 12.0f, 12f, 15.6f)                  // torso
                moveTo(8.2f, 21.2f); lineTo(12f, 15.6f); lineTo(15.8f, 21.2f)  // legs
            }
        }
    }

    /** Lose weight — a flame with a second flame inside it, both as outlines. */
    val Flame: ImageVector by lazy {
        icon("OnbFlame") {
            strokePath(LIMB) {
                moveTo(12f, 2.4f)
                curveTo(12.8f, 5.8f, 19.0f, 8.4f, 19.0f, 14.2f)
                curveTo(19.0f, 18.4f, 15.9f, 21.6f, 12f, 21.6f)
                curveTo(8.1f, 21.6f, 5.0f, 18.4f, 5.0f, 14.2f)
                curveTo(5.0f, 11.2f, 6.5f, 9.1f, 8.4f, 7.4f)
                curveTo(8.4f, 9.2f, 9.2f, 10.4f, 10.5f, 10.8f)
                curveTo(9.9f, 7.6f, 10.8f, 4.8f, 12f, 2.4f)
                close()
                moveTo(12f, 13.0f)                                    // inner flame
                curveTo(13.0f, 14.4f, 14.4f, 15.2f, 14.4f, 16.7f)
                curveTo(14.4f, 18.0f, 13.3f, 18.9f, 12f, 18.9f)
                curveTo(10.7f, 18.9f, 9.6f, 18.0f, 9.6f, 16.7f)
                curveTo(9.6f, 15.3f, 11.0f, 14.4f, 12f, 13.0f)
                close()
            }
        }
    }

    /** General fitness — a heart with a heartbeat running through it. */
    val Heartbeat: ImageVector by lazy {
        icon("OnbHeartbeat") {
            strokePath(LIMB) {
                moveTo(12f, 20.2f)
                curveTo(12f, 20.2f, 3.0f, 15.0f, 3.0f, 8.9f)
                curveTo(3.0f, 6.1f, 5.1f, 4.0f, 7.6f, 4.0f)
                curveTo(9.5f, 4.0f, 11.1f, 5.1f, 12f, 6.6f)
                curveTo(12.9f, 5.1f, 14.5f, 4.0f, 16.4f, 4.0f)
                curveTo(18.9f, 4.0f, 21.0f, 6.1f, 21.0f, 8.9f)
                curveTo(21.0f, 15.0f, 12f, 20.2f, 12f, 20.2f)
                close()
                moveTo(6.2f, 12.2f); lineTo(9.0f, 12.2f); lineTo(10.4f, 9.4f)          // trace
                lineTo(12.8f, 15.0f); lineTo(14.4f, 12.2f); lineTo(17.8f, 12.2f)
            }
        }
    }

    // ── Experience glyphs ─────────────────────────────────────────────────────
    // Three rising bars that fill as experience grows: solid up to the level, ghosted past it, the
    // way a signal meter reads. The first draft outlined the unfilled bars instead, and at 22dp a
    // 4-unit bar outlined at [LIMB] is solid, so all three answers wore the same full meter.

    /** A mass drawn at the ghost rung: still the glyph's colour under any tint, at a third of the ink. */
    private fun ImageVector.Builder.ghostPath(block: PathBuilder.() -> Unit) {
        path(fill = SolidColor(Color.Black), fillAlpha = 0.35f, pathBuilder = block)
    }

    private fun ImageVector.Builder.levelBars(filled: Int) {
        val bars = listOf(
            floatArrayOf(3.0f, 14.0f, 8.0f, 21.0f),
            floatArrayOf(9.5f, 9.0f, 14.5f, 21.0f),
            floatArrayOf(16.0f, 3.0f, 21.0f, 21.0f)
        )
        fillPath { bars.take(filled).forEach { (l, t, r, b) -> roundRect(l, t, r, b, 1.4f) } }
        if (filled < bars.size) {
            ghostPath { bars.drop(filled).forEach { (l, t, r, b) -> roundRect(l, t, r, b, 1.4f) } }
        }
    }

    val Level1: ImageVector by lazy { icon("OnbLevel1") { levelBars(1) } }
    val Level2: ImageVector by lazy { icon("OnbLevel2") { levelBars(2) } }
    val Level3: ImageVector by lazy { icon("OnbLevel3") { levelBars(3) } }

    /** Experience key → glyph. */
    fun forExperience(key: String): ImageVector = when (key) {
        "beginner" -> Level1
        "advanced" -> Level3
        else -> Level2
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
        "get_stronger" -> Lifter
        "lose_weight" -> Flame
        else -> Heartbeat
    }
}

// Vector-builder plumbing (icon/fillPath/strokePath/circle/roundRect) lives in VectorBuilders.kt,
// shared with the other icon families; the glyphs above stay local so this family evolves on its own.
