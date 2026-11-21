package com.forge.app.ui.theme

import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import androidx.compose.material3.MaterialTheme

/**
 * The "Open panel" redesign's own rungs (2026-10-21, Relay designs 7 and 8).
 *
 * The redesign was drawn at 412 wide, so every number here is the design's pixel value read as dp/sp
 * and the build matches it one to one. These sit beside `ForgeTypography` rather than replacing its
 * rungs: screens are moved to the redesign one at a time, and the screens Antho kept as they are
 * (Live session, Freestyle log, Academy) keep the Material scale they were tuned on.
 *
 * The grammar every reading follows: [monoS] label → serif figure ([figureL]/[figureM]) → a small
 * mark → [monoS] change line. Mono labels are written `.uppercase()` at the call site, as §6 asks.
 */
object OpenType {
    /** The label voice: section heads, reading labels, change lines. Design style `mono-s`. */
    val monoS = TextStyle(
        fontFamily = AvexFonts.Monospace,
        fontWeight = FontWeight.Normal,
        fontSize = 11.sp,
        lineHeight = 15.sp,
        letterSpacing = 1.6.sp,
    )

    /** A mono line one step quieter in tracking: change lines and figure captions under a reading. */
    val monoSTight = monoS.copy(letterSpacing = 1.sp)

    /** The small-caption mono (10/1): plan-so-far lines, day-of-week over a date, tick labels. */
    val monoXS = monoS.copy(fontSize = 10.sp, lineHeight = 13.sp, letterSpacing = 1.sp)

    /** Tabs and the top bar's date. Design style `mono-m`. */
    val monoM = TextStyle(
        fontFamily = AvexFonts.Monospace,
        fontWeight = FontWeight.Normal,
        fontSize = 13.sp,
        lineHeight = 17.sp,
        letterSpacing = 1.2.sp,
    )

    /** The one serif hero per screen ("Pull B"). */
    val hero = TextStyle(
        fontFamily = AvexFonts.Serif,
        fontWeight = FontWeight.Normal,
        fontSize = 76.sp,
        lineHeight = 76.sp,
        letterSpacing = (-2.5).sp,
        fontFeatureSettings = "tnum",
    )

    /** A page title in serif (Settings, Profile name). */
    val title = hero.copy(fontSize = 56.sp, lineHeight = 60.sp, letterSpacing = (-1.5).sp)

    /** A reading's figure when two share a row (Sleep, Yesterday). */
    val figureL = TextStyle(
        fontFamily = AvexFonts.Serif,
        fontWeight = FontWeight.Normal,
        fontSize = 44.sp,
        lineHeight = 46.sp,
        fontFeatureSettings = "tnum",
    )

    /** A reading's figure when three share a row (the goal scoreboard). */
    val figureM = figureL.copy(fontSize = 36.sp, lineHeight = 38.sp)

    /** A serif headline inside a section (a project, a block name). */
    val figureMS = figureL.copy(fontSize = 32.sp, lineHeight = 35.sp)

    /** The day number in a list row's date column. */
    val figureS = figureL.copy(fontSize = 22.sp, lineHeight = 24.sp)

    /** The unit after a [figureL] ("h", " bpm", "×5 squat"): half its size, drawn muted. */
    val figureLUnit = figureL.copy(fontSize = 22.sp)

    /** The denominator after a [figureM] ("/8"), as the scoreboard draws it. */
    val figureMDenominator = figureM.copy(fontSize = 20.sp)

    /** The hero's last step for a long answer ("Done for today"), before it wraps. */
    /** Home's day name: a step under [hero], which Antho found a tad big on his phone (2026-10-21). */
    val heroM = hero.copy(fontSize = 64.sp, lineHeight = 66.sp, letterSpacing = (-2).sp)
    val heroS = hero.copy(fontSize = 44.sp, lineHeight = 48.sp, letterSpacing = (-1).sp)

    /** A sheet's serif title ("Show in this spot"). */
    val sheetTitle = figureL.copy(fontSize = 26.sp, lineHeight = 30.sp)

    /** Prose. Design style `body`. */
    val body = TextStyle(
        fontFamily = AvexFonts.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 14.sp,
        lineHeight = 20.sp,
    )

    /** A row's supporting line. */
    val bodyS = body.copy(fontSize = 13.sp, lineHeight = 19.sp)

    /** A list row's title. */
    val rowTitle = TextStyle(
        fontFamily = AvexFonts.SansSerif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 15.sp,
        lineHeight = 20.sp,
    )

    /** A settings row's or tile's title, one step up. Design style `row-title`. */
    val rowTitleL = rowTitle.copy(fontSize = 16.sp, lineHeight = 22.sp)

    /** The primary action's label. White on red only at this size or larger. */
    val button = TextStyle(
        fontFamily = AvexFonts.SansSerif,
        fontWeight = FontWeight.Bold,
        fontSize = 19.sp,
        lineHeight = 24.sp,
    )

    /** A small filled or outlined button's label (Apply, Skip, Undo). Dark text on red. */
    val buttonS = button.copy(fontSize = 14.sp, lineHeight = 18.sp)

    /** Axis ticks and month names under a graph. */
    val chartLabel = TextStyle(
        fontFamily = AvexFonts.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 10.sp,
        lineHeight = 12.sp,
    )

    /** A bar list's row names (muscles), one step up from the axis. */
    val chartRowLabel = chartLabel.copy(fontSize = 11.sp, lineHeight = 14.sp)

    /** A page title, the size Stats and a lift's page open with. */
    val titleL = TextStyle(
        fontFamily = AvexFonts.Serif,
        fontWeight = FontWeight.Normal,
        fontSize = 64.sp,
        lineHeight = 64.sp,
        letterSpacing = (-2).sp,
        fontFeatureSettings = "tnum",
    )

    /** A page title one step down, for a long name (a lift's page). */
    val titleM = titleL.copy(fontSize = 48.sp, lineHeight = 50.sp)

    /** A reading's figure when it leads its section alone (Est. 1RM, body weight). */
    val figureXL = TextStyle(
        fontFamily = AvexFonts.Serif,
        fontWeight = FontWeight.Normal,
        fontSize = 56.sp,
        lineHeight = 58.sp,
        fontFeatureSettings = "tnum",
    )

    /** A figure's unit at half size, for [figureXL] ("lb" after "228"). */
    val figureXLUnit = figureXL.copy(fontSize = 22.sp)

    /** A list row's serif figure (a lift's e1RM on the right). */
    val figureRow = figureXL.copy(fontSize = 28.sp, lineHeight = 30.sp)

    /** A list row's serif set ("195×5") and its reps at a smaller size. */
    val figureSet = figureXL.copy(fontSize = 26.sp, lineHeight = 28.sp)
    val figureSetUnit = figureXL.copy(fontSize = 15.sp)

    /** The PR tag and other 10sp filled tags. */
    val tag = TextStyle(
        fontFamily = AvexFonts.SansSerif,
        fontWeight = FontWeight.ExtraBold,
        fontSize = 10.sp,
        lineHeight = 10.sp,
        letterSpacing = 0.6.sp,
    )
}

/**
 * The redesign's quiet greys. They are data colours, not surfaces: the lit bar is the accent and every
 * other bar, track and rule steps down this ladder, so red is spent only on what moved.
 */
object OpenColors {
    /** An unlit bar that still went up. */
    val Bar = Color(0xFF6A6058)
    /** A bar that went down: dimmer than an unlit one. */
    val BarDown = Color(0xFF3E3731)
    /** Baselines under bar marks, outlined button borders. */
    val Rule = Color(0xFF5A5048)
    /** Meter tracks and rest-day rings (= `PearlOutline`). */
    val Track = PearlOutline
    /** Small glyphs that must stay legible but quiet (rest-day moon). */
    val Glyph = Color(0xFF8E867C)
    /** Unreached steps on a rail. */
    val Unreached = Color(0xFF2A2420)
    /** Dark content on an accent fill, as drawn. */
    val OnAccentDark = Color(0xFF0A0806)
    /** The span before, drawn under the current line on a trend graph. */
    val Previous = Color(0xFF4A423B)
    /** Horizontal bars that are in range (a muscle's weekly sets). */
    val BarQuiet = Color(0xFF4F4740)
    /** Graph gridlines: white at 5%. */
    val Grid = Color(0x0DFFFFFF)
    /** A deload week's band behind a graph: white at 4%. */
    val Band = Color(0x0AFFFFFF)
    /** The deload swatch in a graph's key, a step brighter than the band so it reads at 10dp. */
    val BandKey = Color(0x12FFFFFF)
    /** The edge of a floating readout or sheet: white at 8%. */
    val Edge = Color(0x14FFFFFF)
    /** The drag readout's guide line: muted at 40%. */
    val Guide = Color(0x66BFB6AA)
}

/**
 * The colour of a LARGE label (≥19sp bold) on the accent. The design draws white there; a custom
 * accent light enough to fail 3:1 against white (large-text AA) falls back to the theme's measured
 * on-accent colour instead.
 */
@Composable
@ReadOnlyComposable
fun openOnAccentLarge(): Color {
    val accent = MaterialTheme.colorScheme.primary
    val ratio = (1.05f) / (accent.luminance() + 0.05f)
    return if (ratio >= 3f) Color.White else MaterialTheme.colorScheme.onPrimary
}
