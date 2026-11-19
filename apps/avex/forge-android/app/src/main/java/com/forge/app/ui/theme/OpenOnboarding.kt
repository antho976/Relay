package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/**
 * The rungs and greys the redesigned first run (2026-10-21, Relay "Onboarding" designs) needs beyond
 * [OpenType] and [OpenColors]. Every number is the design's pixel value read as sp/dp, 1:1.
 */
object OnboardingType {
    /** The step's question: serif 36, line height 1.1. */
    val question = OpenType.figureM.copy(fontSize = 36.sp, lineHeight = 1.1.em)

    /** A choice tile's name (Build muscle, Basic gym): sans 16 semibold. */
    val tileTitle = OpenType.rowTitleL

    /** A grid tile's name, one step down so two fit a row at 384dp: sans 15 semibold. */
    val gridTitle = OpenType.rowTitle

    /**
     * A slider's answer ("4 days", "Any length"): serif 22, on the same line as the slider's label.
     * It was a 56sp figure over the days slider and a 40sp one over the length slider, each on a line
     * of its own; Antho found both sections bigger than they needed to be (2026-11-18), and at that
     * size the week preview below them was cut off by the Continue button.
     */
    val sliderFigure = OpenType.figureXL.copy(fontSize = 22.sp, lineHeight = 1.1.em)

    /** Its unit ("days", "min"), muted. */
    val sliderFigureUnit = OpenType.figureXL.copy(fontSize = 13.sp)

    /** A movement's sets by reps on the week page ("3 × 8–12"): serif 16, a step under the name it sits beside. */
    val setsByReps = OpenType.figureXL.copy(fontSize = 16.sp, lineHeight = 1.1.em)

    /** A movement's name on the week page: sans 15 medium. */
    val movement = OpenType.rowTitle.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Medium)

    /** A small choice's label (lb, Male, 15): sans 13 semibold. */
    val chip = OpenType.rowTitle.copy(fontSize = 13.sp, lineHeight = 18.sp)

    /** A typed answer inside a field box: sans 14. */
    val field = OpenType.body

    /** The outlined Re-roll button's label: sans 15 semibold. */
    val secondaryButton = OpenType.rowTitle
}

object OnboardingColors {
    /** The empty track behind a week bar. */
    val BarTrack = Color(0xFF1E1A16)
}
