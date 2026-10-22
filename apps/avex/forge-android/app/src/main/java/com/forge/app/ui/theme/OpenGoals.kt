package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/**
 * The Goals pages' rungs (2026-10-21 redesign, "pg-goals"): the list, Add a goal and Set the target.
 * Read as the design draws them at 412 wide; everything else comes from [OpenType].
 */
object GoalsType {
    /** "Add a goal", a lift's name over its target: a page title one step under [OpenType.title]. */
    val pageTitle = OpenType.title.copy(fontSize = 48.sp, lineHeight = 52.sp, letterSpacing = (-1.2).sp)

    /** A goal's reading on the list ("205"). */
    val figure = OpenType.figureMS.copy(lineHeight = 34.sp)

    /** Its denominator (" / 225 lb"), muted at about half size. */
    val denominator = OpenType.figureMS.copy(fontSize = 15.sp)

    /** The target being set ("225"). */
    val target = OpenType.titleL.copy(letterSpacing = 0.sp, lineHeight = 66.sp)

    /** Its unit ("lb"). */
    val targetUnit = OpenType.titleL.copy(fontSize = 22.sp, letterSpacing = 0.sp)

    /** A reached goal's name in the fold, a step quieter than a live one. */
    val reached = OpenType.rowTitle.copy(fontWeight = FontWeight.Medium)
}

object GoalsColors {
    /** A reached goal's name: ink stepped down, so the fold reads as done rather than live. */
    val Reached = Color(0xFFD9D2C8)
}
