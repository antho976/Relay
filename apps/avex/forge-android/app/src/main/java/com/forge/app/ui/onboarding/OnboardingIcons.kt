package com.forge.app.ui.onboarding

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathBuilder
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp
import com.forge.app.program.Equipment
import com.forge.app.ui.common.fillPath
import com.forge.app.ui.common.icon
import com.forge.app.ui.common.roundRect

/**
 * Glyphs for the equipment, gym-setup and goal tiles in onboarding and Settings.
 *
 * Redrawn 2026-11-18 as one outline set (Relay design 8, page "Icons + days step"): a 24-unit grid,
 * one [STROKE] weight, round caps and joins, no filled masses. The family before this mixed filled
 * silhouettes with thin outlines, and the setup tiles reused three generic glyphs (a house for all
 * three home gyms, a person for both bodyweight setups), so Antho called them old. Now every piece
 * of gear and every setup has its own drawing and nothing repeats.
 *
 * Each glyph is SVG path data in absolute coordinates, the same data the design canvas draws, so a
 * tile here and its artboard there show one shape. Rendered muted via `Icon(tint = ...)`.
 * Every [Equipment] value has a glyph: [forEquipment] is exhaustive so a new enum entry fails loudly
 * here instead of silently rendering a blank tile.
 */
object OnboardingIcons {

    /** The one line weight of the family, in viewport units. */
    private const val STROKE = 1.75f

    private fun outline(name: String, pathData: String): ImageVector =
        ImageVector.Builder(
            name = name,
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).addPath(
            pathData = addPathNodes(pathData),
            fill = null,
            stroke = SolidColor(Color.Black),
            strokeLineWidth = STROKE,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        ).build()

    /** Dumbbell, tilted 45°: level, a dumbbell at 24dp is the letter H and sits too close to [Barbell]. */
    val Dumbbell: ImageVector by lazy {
        outline("OnbDumbbell", "M9.88 14.12 L14.12 9.88 M4.58 12.35 L5.64 11.29 A1 1 -45 0 1 7.05 11.29 L12.71 16.95 A1 1 -45 0 1 12.71 18.36 L11.65 19.42 A1 1 -45 0 1 10.23 19.42 L4.58 13.77 A1 1 -45 0 1 4.58 12.35 z M11.29 5.64 L12.35 4.58 A1 1 -45 0 1 13.77 4.58 L19.42 10.23 A1 1 -45 0 1 19.42 11.65 L18.36 12.71 A1 1 -45 0 1 16.95 12.71 L11.29 7.05 A1 1 -45 0 1 11.29 5.64 z M5.64 14.83 L4.22 16.24 A1 1 -45 0 0 4.22 17.66 L6.34 19.78 A1 1 -45 0 0 7.76 19.78 L9.17 18.36 M14.83 5.64 L16.24 4.22 A1 1 -45 0 1 17.66 4.22 L19.78 6.34 A1 1 -45 0 1 19.78 7.76 L18.36 9.17")
    }

    /** Barbell, side on: a big and a small plate each side, the sleeve showing past them. */
    val Barbell: ImageVector by lazy {
        outline("OnbBarbell", "M9 12 L15 12 M2 12 L4 12 M20 12 L22 12 M7.5 5.5 L8 5.5 A1 1 0 0 1 9 6.5 L9 17.5 A1 1 0 0 1 8 18.5 L7.5 18.5 A1 1 0 0 1 6.5 17.5 L6.5 6.5 A1 1 0 0 1 7.5 5.5 z M16 5.5 L16.5 5.5 A1 1 0 0 1 17.5 6.5 L17.5 17.5 A1 1 0 0 1 16.5 18.5 L16 18.5 A1 1 0 0 1 15 17.5 L15 6.5 A1 1 0 0 1 16 5.5 z M6.5 8 L5 8 A1 1 0 0 0 4 9 L4 15 A1 1 0 0 0 5 16 L6.5 16 M17.5 8 L19 8 A1 1 0 0 1 20 9 L20 15 A1 1 0 0 1 19 16 L17.5 16")
    }

    /** Squat rack: two uprights under a top beam, a loaded bar across the hooks. */
    val SquatRack: ImageVector by lazy {
        outline("OnbSquatRack", "M6 21 L6 3 L18 3 L18 21 M2 9.5 L22 9.5 M3.5 7 L3.5 12 M20.5 7 L20.5 12 M4 21 L8 21 M16 21 L20 21")
    }

    /** Smith machine: the same frame, but the bar stays inside it on two carriages. */
    val Smith: ImageVector by lazy {
        outline("OnbSmith", "M6 21 L6 3 L18 3 L18 21 M7.5 12 L16.5 12 M4.5 12 A1.5 1.5 0 1 0 7.5 12 A1.5 1.5 0 1 0 4.5 12 M16.5 12 A1.5 1.5 0 1 0 19.5 12 A1.5 1.5 0 1 0 16.5 12 M4 21 L8 21 M16 21 L20 21")
    }

    /** Trap / hex bar from above: the hexagon you stand in, a sleeve and a plate each side. */
    val TrapBar: ImageVector by lazy {
        outline("OnbTrapBar", "M2 12 L8 12 L10.5 7.5 L13.5 7.5 L16 12 L22 12 M8 12 L10.5 16.5 L13.5 16.5 L16 12 M4.5 8.5 L4.5 15.5 M19.5 8.5 L19.5 15.5")
    }

    /** EZ-bar: the bent grip between two plates. */
    val EzBar: ImageVector by lazy {
        outline("OnbEzBar", "M2 12 L5 12 M19 12 L22 12 M7.5 12 L9 12 L10.5 9.5 L13.5 14.5 L15 12 L16.5 12 M6 7.5 L6.5 7.5 A1 1 0 0 1 7.5 8.5 L7.5 15.5 A1 1 0 0 1 6.5 16.5 L6 16.5 A1 1 0 0 1 5 15.5 L5 8.5 A1 1 0 0 1 6 7.5 z M17.5 7.5 L18 7.5 A1 1 0 0 1 19 8.5 L19 15.5 A1 1 0 0 1 18 16.5 L17.5 16.5 A1 1 0 0 1 16.5 15.5 L16.5 8.5 A1 1 0 0 1 17.5 7.5 z")
    }

    /** Kettlebell: a ball under a handle. */
    val Kettlebell: ImageVector by lazy {
        outline("OnbKettlebell", "M8.6 9.4 L8.6 6.5 A3.4 3.4 0 0 1 15.4 6.5 L15.4 9.4 M12 8.5 A6.25 6.25 0 1 0 12 21 A6.25 6.25 0 1 0 12 8.5 z")
    }

    /** Resistance band: a tube between two stirrup handles. */
    val Band: ImageVector by lazy {
        outline("OnbBand", "M7 9.5 L7 15 A5 5 0 0 0 17 15 L17 9.5 M7 9.5 L3.75 4.5 L10.25 4.5 z M17 9.5 L13.75 4.5 L20.25 4.5 z")
    }

    /** Cable machine: an upright, the pulley at its arm, the cable down to a handle. */
    val Cable: ImageVector by lazy {
        outline("OnbCable", "M6 21 L6 4 A1 1 0 0 1 7 3 L13.75 3 M13.75 4.5 A1.75 1.75 0 1 0 17.25 4.5 A1.75 1.75 0 1 0 13.75 4.5 M17.25 4.5 L17.25 13 M17.25 13 L14.75 16.5 L19.75 16.5 z M3 21 L9 21")
    }

    /** Pull-up bar, with someone hanging from it: alone, a bar is a line. */
    val PullUpBar: ImageVector by lazy {
        outline("OnbPullUpBar", "M3.5 5 L20.5 5 M8.5 5 L10.5 10.5 M15.5 5 L13.5 10.5 M10.25 8.25 A1.75 1.75 0 1 0 13.75 8.25 A1.75 1.75 0 1 0 10.25 8.25 M10.5 10.5 L13.5 10.5 M12 10.5 L12 16.5 M12 16.5 L10 21 M12 16.5 L14 21")
    }

    /** Flat bench, side on. */
    val Bench: ImageVector by lazy {
        outline("OnbBench", "M4 9 L20 9 A1 1 0 0 1 21 10 L21 11 A1 1 0 0 1 20 12 L4 12 A1 1 0 0 1 3 11 L3 10 A1 1 0 0 1 4 9 z M6.5 12 L6.5 18.5 M17.5 12 L17.5 18.5 M4.5 18.5 L8.5 18.5 M15.5 18.5 L19.5 18.5")
    }

    /** Incline bench: the seat, and the back pad raised on its strut. */
    val InclineBench: ImageVector by lazy {
        outline("OnbInclineBench", "M3.5 13 L8.5 13 A1 1 0 0 1 9.5 14 L9.5 14.5 A1 1 0 0 1 8.5 15.5 L3.5 15.5 A1 1 0 0 1 2.5 14.5 L2.5 14 A1 1 0 0 1 3.5 13 z M10.5 13.6 L18.2 4.8 L20.1 6.4 L12.4 15.2 z M6 15.5 L6 19.5 M4 19.5 L20 19.5 M16 19.5 L16 11.1")
    }

    /** Dip bars: two posts with the handles turned in. */
    val DipStation: ImageVector by lazy {
        outline("OnbDipStation", "M5 21 L5 8 A2 2 0 0 1 7 6 L10.5 6 M19 21 L19 8 A2 2 0 0 0 17 6 L13.5 6 M3 21 L7 21 M17 21 L21 21")
    }

    /** Suspension trainer: two straps from one anchor down to rings. */
    val Suspension: ImageVector by lazy {
        outline("OnbSuspension", "M10 3 L14 3 M12 3 L7 14 M12 3 L17 14 M4 17 A3 3 0 1 0 10 17 A3 3 0 1 0 4 17 M14 17 A3 3 0 1 0 20 17 A3 3 0 1 0 14 17")
    }

    /** Ab wheel: the wheel on its axle handles. */
    val AbWheel: ImageVector by lazy {
        outline("OnbAbWheel", "M6 12 A6 6 0 1 0 18 12 A6 6 0 1 0 6 12 M2 12 L6 12 M18 12 L22 12 M10.75 12 A1.25 1.25 0 1 0 13.25 12 A1.25 1.25 0 1 0 10.75 12")
    }

    /** Bodyweight: a figure mid jumping jack. */
    val Bodyweight: ImageVector by lazy {
        outline("OnbBodyweight", "M10 4 A2 2 0 1 0 14 4 A2 2 0 1 0 10 4 M12 9.5 L6.5 5.5 M12 9.5 L17.5 5.5 M12 9.5 L12 15 M12 15 L8.5 21 M12 15 L15.5 21")
    }

    /** Machine: a weight stack in its frame, the pin through one plate. */
    val Machine: ImageVector by lazy {
        outline("OnbMachine", "M7 21 L7 4 A1 1 0 0 1 8 3 L16 3 A1 1 0 0 1 17 4 L17 21 M4.5 21 L19.5 21 M12 3 L12 9.5 M9.75 9.5 L14.25 9.5 M9.75 12.25 L14.25 12.25 M9.75 15 L14.25 15 M9.75 17.75 L14.25 17.75 M14.25 12.25 L19.25 12.25")
    }

    /** Everything gym: a building with a barbell over the door. */
    val Building: ImageVector by lazy {
        outline("OnbBuilding", "M4.5 21 L4.5 6 A1 1 0 0 1 5.5 5 L18.5 5 A1 1 0 0 1 19.5 6 L19.5 21 M2.5 21 L21.5 21 M10 21 L10 16.5 L14 16.5 L14 21 M6.75 10.5 L17.25 10.5 M8.75 8.5 L8.75 12.5 M15.25 8.5 L15.25 12.5 M10.5 9.25 L10.5 11.75 M13.5 9.25 L13.5 11.75")
    }

    /** Basic gym: a bench under a racked bar. */
    val BenchStation: ImageVector by lazy {
        outline("OnbBenchStation", "M7.5 20.5 L7.5 7 M16.5 20.5 L16.5 7 M2.5 8.5 L21.5 8.5 M4.5 5.5 L4.5 11.5 M19.5 5.5 L19.5 11.5 M9.5 14.5 L14.5 14.5 M12 14.5 L12 20.5")
    }

    /** Home gym, big: a house with a barbell in it. */
    val HouseBarbell: ImageVector by lazy {
        outline("OnbHouseBarbell", "M3 10.5 L12 3 L21 10.5 L21 20 A1 1 0 0 1 20 21 L4 21 A1 1 0 0 1 3 20 z M6.5 15.5 L17.5 15.5 M9.25 12.75 L9.25 18.25 M14.75 12.75 L14.75 18.25 M7.25 14.25 L7.25 16.75 M16.75 14.25 L16.75 16.75")
    }

    /** Home gym, small: a house with one dumbbell in it. */
    val HouseDumbbell: ImageVector by lazy {
        outline("OnbHouseDumbbell", "M3 10.5 L12 3 L21 10.5 L21 20 A1 1 0 0 1 20 21 L4 21 A1 1 0 0 1 3 20 z M10.76 16.74 L13.24 14.26 M8.29 16.38 L11.12 19.21 M12.88 11.79 L15.71 14.62")
    }

    /** Home machine gym: a house with a weight stack in it. */
    val HouseMachine: ImageVector by lazy {
        outline("OnbHouseMachine", "M3 10.5 L12 3 L21 10.5 L21 20 A1 1 0 0 1 20 21 L4 21 A1 1 0 0 1 3 20 z M9 13.5 L15 13.5 M9 16 L15 16 M9 18.5 L15 18.5 M12 11 L12 13.5")
    }

    /** Build muscle: a flexed arm. */
    val Muscle: ImageVector by lazy {
        outline("OnbMuscle", "M12.41 13.02 A5 5 0 0 1 22 15 C22 18.87 18 22 13 22 C8.92 22 4.85 21.18 2.63 19.54 C2.2 19.22 2 18.71 2.01 18.18 C2.12 12.72 2.63 2 10 2 A3 3 0 0 1 13 5 A2 2 0 0 1 11 7 C9.89 7 9.36 6.56 9 6 M15 14 A5 5 0 0 0 7.42 16 M9.96 6.83 C8.02 7.98 9.5 13 8 15")
    }

    /** Get stronger: a loaded bar and an arrow up. */
    val BarUp: ImageVector by lazy {
        outline("OnbBarUp", "M12 9 L12 3 M9 5.75 L12 2.75 L15 5.75 M9.5 16 L14.5 16 M2 16 L3.5 16 M20.5 16 L22 16 M7.5 11 L8.5 11 A1 1 0 0 1 9.5 12 L9.5 20 A1 1 0 0 1 8.5 21 L7.5 21 A1 1 0 0 1 6.5 20 L6.5 12 A1 1 0 0 1 7.5 11 z M15.5 11 L16.5 11 A1 1 0 0 1 17.5 12 L17.5 20 A1 1 0 0 1 16.5 21 L15.5 21 A1 1 0 0 1 14.5 20 L14.5 12 A1 1 0 0 1 15.5 11 z M6.5 13 L4.5 13 A1 1 0 0 0 3.5 14 L3.5 18 A1 1 0 0 0 4.5 19 L6.5 19 M17.5 13 L19.5 13 A1 1 0 0 1 20.5 14 L20.5 18 A1 1 0 0 1 19.5 19 L17.5 19")
    }

    /** Lose weight: a bathroom scale. */
    val Scale: ImageVector by lazy {
        outline("OnbScale", "M6 3.5 L18 3.5 A2.5 2.5 0 0 1 20.5 6 L20.5 18 A2.5 2.5 0 0 1 18 20.5 L6 20.5 A2.5 2.5 0 0 1 3.5 18 L3.5 6 A2.5 2.5 0 0 1 6 3.5 z M8 11 A4 4 0 0 1 16 11 z M12 11 L13.6 8.6")
    }

    /** General fitness: a heart with a pulse through it. */
    val Heartbeat: ImageVector by lazy {
        outline("OnbHeartbeat", "M20.84 4.61 A5.5 5.5 0 0 0 13.06 4.61 L12 5.67 L10.94 4.61 A5.5 5.5 0 0 0 3.16 12.39 L4.22 13.45 L12 21.23 L19.78 13.45 L20.84 12.39 A5.5 5.5 0 0 0 20.84 4.61 z M7 12 L9.5 12 L11 9.5 L13 14 L14.5 12 L17 12")
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
        "basic-gym" -> BenchStation
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
        "get_stronger" -> BarUp
        "lose_weight" -> Scale
        else -> Heartbeat
    }
}

// Vector-builder plumbing (icon/fillPath/strokePath/circle/roundRect) lives in VectorBuilders.kt,
// shared with the other icon families; the glyphs above stay local so this family evolves on its own.
