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

    /** The few lines that carry a body rather than an edge — a raised back pad, a torso. */
    private const val LIMB_BOLD = 2.4f

    /** Dumbbell — two deep bells on a thick handle. The bells are wider than the handle is tall,
     *  which is the whole difference between a dumbbell and the letter H. */
    val Dumbbell: ImageVector by lazy {
        icon("OnbDumbbell") {
            fillPath {
                roundRect(7.6f, 11.0f, 16.4f, 13.0f, 1.0f)   // handle
                roundRect(4.2f, 5.8f, 8.2f, 18.2f, 1.8f)     // left bell
                roundRect(15.8f, 5.8f, 19.8f, 18.2f, 1.8f)   // right bell
            }
        }
    }

    /** Barbell — a full-width bar with a plate and a collar out each side. Longer bar and four
     *  masses instead of two is what separates it from the dumbbell at this size. */
    val Barbell: ImageVector by lazy {
        icon("OnbBarbell") {
            fillPath {
                roundRect(1.4f, 10.8f, 22.6f, 13.2f, 1.2f)   // bar
                roundRect(5.4f, 6.2f, 8.4f, 17.8f, 1.3f)     // left plate
                roundRect(15.6f, 6.2f, 18.6f, 17.8f, 1.3f)   // right plate
                roundRect(2.8f, 8.6f, 5.0f, 15.4f, 1.0f)     // left collar
                roundRect(19.0f, 8.6f, 21.2f, 15.4f, 1.0f)   // right collar
            }
        }
    }

    /** Squat / power rack — two uprights on a base, holding a bar in their J-hooks. Bottom-heavy,
     *  which is what tells it apart from the top-hung [Smith] at 24dp. */
    val SquatRack: ImageVector by lazy {
        icon("OnbRack") {
            fillPath {
                roundRect(5.6f, 5.0f, 7.4f, 19.2f, 0.9f)     // left upright
                roundRect(16.6f, 5.0f, 18.4f, 19.2f, 0.9f)   // right upright
                roundRect(3.4f, 19.2f, 9.6f, 20.8f, 0.8f)    // left foot
                roundRect(14.4f, 19.2f, 20.6f, 20.8f, 0.8f)  // right foot
                roundRect(2.2f, 8.2f, 21.8f, 10.0f, 0.9f)    // racked bar
                roundRect(7.4f, 10.0f, 9.2f, 11.8f, 0.7f)    // left hook
                roundRect(14.8f, 10.0f, 16.6f, 11.8f, 0.7f)  // right hook
            }
        }
    }

    /** Smith machine — the same frame with the bar captured on the rails, running the full width.
     *  Hooks say rack, a bar through the uprights says Smith. */
    val Smith: ImageVector by lazy {
        icon("OnbSmith") {
            fillPath {
                roundRect(5.2f, 3.8f, 7.0f, 20.2f, 0.9f)     // left rail
                roundRect(17.0f, 3.8f, 18.8f, 20.2f, 0.9f)   // right rail
                roundRect(3.4f, 3.8f, 20.6f, 5.6f, 0.9f)     // top beam
                roundRect(7.0f, 12.6f, 17.0f, 14.4f, 0.9f)   // captured bar, inside the rails
            }
        }
    }

    /** Trap / hex bar — the hexagonal frame seen from above, with a loading sleeve out each side. */
    val TrapBar: ImageVector by lazy {
        icon("OnbTrapBar") {
            strokePath(LIMB) {
                moveTo(12f, 5.4f); lineTo(18.2f, 9.1f); lineTo(18.2f, 14.9f)
                lineTo(12f, 18.6f); lineTo(5.8f, 14.9f); lineTo(5.8f, 9.1f); close()
                moveTo(1.9f, 12f); lineTo(5.8f, 12f)
                moveTo(18.2f, 12f); lineTo(22.1f, 12f)
            }
        }
    }

    /** EZ-bar — the zig-zag curl bar with a plate on each end. */
    val EzBar: ImageVector by lazy {
        icon("OnbEzBar") {
            strokePath(LIMB) {
                moveTo(4.4f, 12f); lineTo(7.0f, 12f)
                lineTo(9.6f, 9.3f); lineTo(14.4f, 14.7f); lineTo(17.0f, 12f)
                lineTo(19.6f, 12f)
            }
            fillPath {
                roundRect(1.6f, 8.6f, 4.4f, 15.4f, 1.1f)
                roundRect(19.6f, 8.6f, 22.4f, 15.4f, 1.1f)
            }
        }
    }

    /** Kettlebell — a deep filled bell under a stroked handle. */
    val Kettlebell: ImageVector by lazy {
        icon("OnbKettlebell") {
            strokePath(LIMB) {
                moveTo(8.6f, 10.4f)
                curveTo(8.6f, 3.6f, 15.4f, 3.6f, 15.4f, 10.4f)
            }
            fillPath { circle(12f, 15.2f, 5.9f) }
        }
    }

    /** Resistance band — an arch of band under tension down to two grips. Two rings joined by a
     *  curve is the infinity sign, which is what the first version drew. */
    val Band: ImageVector by lazy {
        icon("OnbBand") {
            strokePath(LIMB) {
                moveTo(4.4f, 12f)
                curveTo(7.2f, 3.4f, 10.4f, 20.6f, 13.2f, 12f)  // the band, stretched
                curveTo(15.0f, 6.6f, 17.4f, 15.8f, 19.2f, 12f)
            }
            fillPath {
                circle(3.6f, 12f, 2.0f)                        // left grip
                circle(20.4f, 12f, 2.0f)                       // right grip
            }
        }
    }

    /**
     * Cable machine — the classic crossover frame head-on: two uprights under a top beam, the weight
     * stack on the floor between them, and a handle hanging on a short cable from each top corner.
     * The handles sit well above the stack on purpose: at tile size the interior only reads if the
     * parts have air between them. Same frame idiom as [SquatRack] and [Smith], but full and solid
     * inside where theirs are open.
     */
    val Cable: ImageVector by lazy {
        icon("OnbCable") {
            fillPath {
                roundRect(2.4f, 3.2f, 4.8f, 20.8f, 1.0f)     // left upright
                roundRect(19.2f, 3.2f, 21.6f, 20.8f, 1.0f)   // right upright
                roundRect(2.4f, 3.2f, 21.6f, 5.5f, 1.0f)     // top beam
                roundRect(9.0f, 14.6f, 15.0f, 17.2f, 0.8f)   // weight stack
                roundRect(9.0f, 18.2f, 15.0f, 20.8f, 0.8f)
                roundRect(5.3f, 9.8f, 8.5f, 11.8f, 1.0f)     // left handle
                roundRect(15.5f, 9.8f, 18.7f, 11.8f, 1.0f)   // right handle
            }
            strokePath(1.8f) {
                moveTo(6.9f, 5.5f); lineTo(6.9f, 9.8f)       // left cable
                moveTo(17.1f, 5.5f); lineTo(17.1f, 9.8f)     // right cable
            }
        }
    }

    /**
     * Pull-up bar — the bar with someone hanging off it. Drawn as the hang and not as the hardware
     * on purpose: a beam on two uprights is [SquatRack] and [Smith] already, and uprights flaring
     * into grips is [DipStation], so the object alone has no silhouette left of its own. It shares
     * the figure idiom with [Bodyweight] but reads apart from it — arms up into a bar, not splayed.
     */
    val PullUpBar: ImageVector by lazy {
        icon("OnbPullUp") {
            fillPath {
                roundRect(2.0f, 3.2f, 22.0f, 5.5f, 1.15f)    // the bar
                circle(6.2f, 5.9f, 1.45f)                    // left hand
                circle(17.8f, 5.9f, 1.45f)                   // right hand
                circle(12f, 8.8f, 1.85f)                     // head
            }
            strokePath(2.0f) {
                moveTo(6.2f, 5.9f); lineTo(10.4f, 12.8f)     // left arm
                moveTo(17.8f, 5.9f); lineTo(13.6f, 12.8f)    // right arm
                moveTo(12f, 10.5f); lineTo(12f, 15.8f)       // torso
                moveTo(12f, 15.8f); lineTo(9.5f, 20.4f)      // left leg
                moveTo(12f, 15.8f); lineTo(14.5f, 20.4f)     // right leg
            }
        }
    }

    /** Flat bench — side view: one thick pad on two legs. */
    val Bench: ImageVector by lazy {
        icon("OnbBench") {
            fillPath {
                roundRect(3.2f, 9.4f, 20.8f, 12.3f, 1.4f)    // pad
                roundRect(6.2f, 12.3f, 8.6f, 19.4f, 1.0f)    // near leg
                roundRect(15.4f, 12.3f, 17.8f, 19.4f, 1.0f)  // far leg
            }
        }
    }

    /** Incline bench — the same pad with the back raised, which is the only thing to read here. */
    val InclineBench: ImageVector by lazy {
        icon("OnbIncline") {
            strokePath(LIMB_BOLD) { moveTo(4.6f, 5.6f); lineTo(10.8f, 13.2f) }   // raised back
            fillPath {
                roundRect(9.6f, 12.4f, 20.8f, 15.3f, 1.4f)   // seat
                roundRect(11.8f, 15.3f, 14.2f, 19.6f, 1.0f)  // near leg
                roundRect(17.4f, 15.3f, 19.8f, 19.6f, 1.0f)  // far leg
            }
        }
    }

    /** Bodyweight — a figure with nothing in its hands. Drawn at the bold limb so it carries the
     *  same weight as the equipment it sits beside rather than reading as wireframe. */
    val Bodyweight: ImageVector by lazy {
        icon("OnbBodyweight") {
            fillPath { circle(12f, 5.0f, 2.4f) }
            strokePath(LIMB_BOLD) {
                moveTo(12f, 7.8f); lineTo(12f, 14.2f)          // torso
                moveTo(12f, 10.0f); lineTo(6.4f, 6.0f)         // left arm, raised
                moveTo(12f, 10.0f); lineTo(17.6f, 6.0f)        // right arm, raised
                moveTo(12f, 14.2f); lineTo(8.2f, 20.4f)        // left leg
                moveTo(12f, 14.2f); lineTo(15.8f, 20.4f)       // right leg
            }
        }
    }

    /** Dip bars — head-on: two posts with the grip bars running out to each side. */
    val DipStation: ImageVector by lazy {
        icon("OnbDip") {
            fillPath {
                roundRect(8.0f, 9.6f, 10.4f, 20.4f, 1.1f)     // left post
                roundRect(13.6f, 9.6f, 16.0f, 20.4f, 1.1f)    // right post
                roundRect(2.0f, 7.2f, 10.4f, 9.6f, 1.1f)      // left grip
                roundRect(13.6f, 7.2f, 22.0f, 9.6f, 1.1f)     // right grip
            }
        }
    }

    /** Suspension trainer — a wide anchor with the straps splaying out to two handles. Straps
     *  meeting at a point drew the letter A, which is not a thing you hang from. */
    val Suspension: ImageVector by lazy {
        icon("OnbSuspension") {
            fillPath { roundRect(9.2f, 2.8f, 14.8f, 5.0f, 1.1f) }     // anchor
            strokePath(LIMB) {
                moveTo(11.0f, 5.0f); lineTo(6.6f, 13.6f)              // left strap
                moveTo(13.0f, 5.0f); lineTo(17.4f, 13.6f)             // right strap
                circle(5.8f, 16.6f, 3.0f)                             // left handle loop
                circle(18.2f, 16.6f, 3.0f)                            // right handle loop
            }
        }
    }

    /** Ab wheel — side view: the wheel with its axle handle out each side. */
    val AbWheel: ImageVector by lazy {
        icon("OnbAbWheel") {
            strokePath(LIMB) { circle(12f, 12f, 5.6f) }
            fillPath { circle(12f, 12f, 1.5f) }
            strokePath(LIMB) {
                moveTo(2.2f, 12f); lineTo(6.4f, 12f)
                moveTo(17.6f, 12f); lineTo(21.8f, 12f)
            }
        }
    }

    /** Machine — a selectorized stack hanging off its top beam. */
    val Machine: ImageVector by lazy {
        icon("OnbMachine") {
            fillPath {
                roundRect(5.6f, 3.6f, 18.4f, 5.8f, 1.1f)     // top beam
                roundRect(10.9f, 5.8f, 13.1f, 9.6f, 0.8f)    // cable
                roundRect(6.4f, 9.6f, 17.6f, 12.4f, 0.9f)    // plate
                roundRect(6.4f, 13.4f, 17.6f, 16.2f, 0.9f)   // plate
                roundRect(6.4f, 17.2f, 17.6f, 20.0f, 0.9f)   // plate
            }
        }
    }

    /** House — the home-gym presets. */
    val House: ImageVector by lazy {
        icon("OnbHouse") {
            strokePath(LIMB) {
                moveTo(3.0f, 11.6f); lineTo(12f, 4.0f); lineTo(21.0f, 11.6f)
                moveTo(5.4f, 10.4f); lineTo(5.4f, 19.8f); lineTo(18.6f, 19.8f); lineTo(18.6f, 10.4f)
            }
            fillPath { roundRect(10.0f, 14.2f, 14.0f, 19.8f, 0.8f) }   // door
        }
    }

    /** Building — the commercial full-gym preset. Four windows, not six: at 24dp the third row
     *  turned the facade into texture. */
    val Building: ImageVector by lazy {
        icon("OnbBuilding") {
            strokePath(LIMB) { roundRect(4.6f, 3.8f, 19.4f, 20.2f, 1.6f) }
            fillPath {
                roundRect(7.8f, 7.4f, 10.6f, 10.2f, 0.6f)
                roundRect(13.4f, 7.4f, 16.2f, 10.2f, 0.6f)
                roundRect(7.8f, 11.8f, 10.6f, 14.6f, 0.6f)
                roundRect(13.4f, 11.8f, 16.2f, 14.6f, 0.6f)
                roundRect(10.0f, 16.2f, 14.0f, 20.2f, 0.7f)  // door
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
        "home-big" -> SquatRack
        "home-small" -> Bench
        "developer" -> House
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
