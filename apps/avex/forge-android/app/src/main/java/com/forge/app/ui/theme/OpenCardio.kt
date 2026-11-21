package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/**
 * The Cardio screens' rungs beyond [OpenType] (redesign 2026-10-21, page "pg-cardio"). Each one is a
 * size the Cardio designs draw that the shared ladder does not carry; everything else on those
 * screens reads straight off [OpenType] and [OpenColors].
 */
object CardioOpenType {
    /** A figure's unit when three readings share a row (36 → 18): "8.8 mi", "40 min". */
    val figureMUnit = OpenType.figureM.copy(fontSize = 18.sp)

    /** A session row's serif figure on the right ("9:31 /mi", "95 min" on a week row). */
    val rowFigure = OpenType.figureXL.copy(fontSize = 24.sp, lineHeight = 26.sp)

    /** The unit after [rowFigure], about half its size. */
    val rowFigureUnit = OpenType.figureXL.copy(fontSize = 13.sp)

    /** A week row's unit ("min"), a touch smaller than [rowFigureUnit]. */
    val rowFigureUnitS = OpenType.figureXL.copy(fontSize = 12.sp)

    /** The log sheet's duration reading ("30 min"). */
    val sheetFigure = OpenType.figureXL.copy(fontSize = 28.sp, lineHeight = 30.sp)

    /** The unit after [sheetFigure]. */
    val sheetFigureUnit = OpenType.figureXL.copy(fontSize = 14.sp)

    /** The duration quick picks: mono, a step over [OpenType.monoM]. */
    val pick = OpenType.monoM.copy(fontSize = 14.sp, lineHeight = 18.sp, letterSpacing = 0.sp)

    /** A typed value inside an outlined field (the distance). */
    val fieldValue = OpenType.body.copy(fontSize = 15.sp, lineHeight = 20.sp)

    /** The watch chip riding the Log cardio bar ("1 from watch"). */
    val chip = OpenType.body.copy(fontSize = 13.sp, lineHeight = 16.sp, fontWeight = FontWeight.SemiBold)
}

/** The Cardio screens' colours beyond [OpenColors]. */
object CardioOpenColors {
    /** The dark wash under the "1 from watch" chip on the red Log cardio bar: black at 22%. */
    val ChipWash = Color(0x38000000)

    /** The outline on the log sheet's quiet chips and fields, as drawn (`#5A5048`). */
    val FieldEdge = OpenColors.Rule

    /** An unreached day stub on a week row's mini bars (`#2A2420`). */
    val DayStub = OpenColors.Unreached

    /** The "every run" ticks on a session's strip plots (`#6A6058`). */
    val Tick = OpenColors.Bar
}
