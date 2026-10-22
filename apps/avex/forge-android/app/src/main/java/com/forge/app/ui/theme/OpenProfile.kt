package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.sp

/**
 * The Profile page's own rungs (2026-10-21 redesign, Relay "pf1 / pf2 / ph / lk4"). Everything not
 * here is drawn with [OpenType] and [OpenColors].
 */
object ProfileType {
    /** The unit after the lifted figure ("lb lifted"), at about a third of its 44sp. */
    val liftedUnit = OpenType.figureL.copy(fontSize = 16.sp)

    /** A count when three share a row under All time (workouts, sets, PRs). */
    val count = OpenType.figureL.copy(fontSize = 30.sp, lineHeight = 32.sp)

    /** The unit after a composition figure ("%", "lb", "in"). */
    val compositionUnit = OpenType.figureMS.copy(fontSize = 14.sp)

    /** The composition figure's steps down when a narrow column will not set it on one line. */
    val compositionStep = OpenType.figureMS.copy(fontSize = 28.sp, lineHeight = 31.sp)
    val compositionSmall = OpenType.figureMS.copy(fontSize = 24.sp, lineHeight = 27.sp)
    val compositionUnitStep = OpenType.figureMS.copy(fontSize = 12.sp)

    /** A weekday letter over the month grid. */
    val weekday = OpenType.monoXS.copy(letterSpacing = 0.sp)
}

object ProfileColors {
    /** The cover's placeholder ground before a photo is chosen, top to bottom. */
    val CoverTop = Color(0xFF3A3029)
    val CoverMid = Color(0xFF241D18)
    val CoverBottom = Color(0xFF17120E)
    val coverPlaceholder: Brush get() = Brush.verticalGradient(0f to CoverTop, 0.6f to CoverMid, 1f to CoverBottom)

    /**
     * The mask that dissolves a cover photo into the page: faded under the status bar and the top
     * bar's glyphs, solid through the middle, then an eased tail to nothing under the name, so the
     * photo never ends on a hard edge. Used with BlendMode.DstIn.
     */
    val coverFade: Brush = Brush.verticalGradient(
        0f to Color.Transparent,
        0.22f to Color.Black,
        0.50f to Color.Black,
        0.62f to Color(0xB8000000),
        0.72f to Color(0x66000000),
        0.82f to Color(0x2E000000),
        0.91f to Color(0x0F000000),
        1f to Color.Transparent
    )

    /** A past day on the month grid that was not trained. */
    val DayPast = Color(0xFF1E1A16)

    /** A photo frame's ground while it decodes, and the empty frames on an empty gallery. */
    val PhotoTop = Color(0xFF3A3029)
    val PhotoBottom = Color(0xFF1A1613)
    val photoGround: Brush get() = Brush.verticalGradient(listOf(PhotoTop, PhotoBottom))

    /** The person glyph inside an empty photo frame. */
    val PhotoGlyph = Color(0xFF5A5048)

    /** The hidden tiles where locked photos would be. */
    val Hidden = Color(0xFF1A1613)

    /** The shade under a photo's caption so the date reads over any picture. */
    val CaptionShade = Color(0xA6000000)

    /** The haloes behind the name over a cover photo. */
    val NameHalo = Color(0xA6000000)
}
