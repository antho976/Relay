package com.forge.app.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import com.forge.app.R

/**
 * The three voices as bundled faces, not the phone's (2026-10-21 redesign).
 *
 * `FontFamily.Serif` / `SansSerif` / `Monospace` resolve to whatever the phone ships: Noto Serif and
 * Roboto on a Pixel, something else on a Samsung. The redesign was drawn in Noto Serif Display, Noto
 * Sans and Noto Sans Mono, and the serif in particular is the Display cut (higher contrast, tighter
 * at hero sizes), which no phone ships. Bundling them is what makes the build match the designs on
 * every device. `res/font` holds Latin-only subsets (about 470 KB together); a character outside
 * them (↑, CJK) falls back to the system face per glyph. All three are SIL OFL 1.1 and keep their
 * licence in the font metadata; About → Open-source licenses names them.
 *
 * Semibold maps to the Bold file on purpose: the designs were rendered with no Noto Sans SemiBold
 * installed, so every 600 in them was drawn Bold, and Bold is what was approved.
 */
object AvexFonts {
    val Serif = FontFamily(Font(R.font.noto_serif_display_regular, FontWeight.Normal))
    val SansSerif = FontFamily(
        Font(R.font.noto_sans_regular, FontWeight.Normal),
        Font(R.font.noto_sans_medium, FontWeight.Medium),
        Font(R.font.noto_sans_bold, FontWeight.SemiBold),
        Font(R.font.noto_sans_bold, FontWeight.Bold),
        Font(R.font.noto_sans_black, FontWeight.ExtraBold),
        Font(R.font.noto_sans_black, FontWeight.Black),
    )
    val Monospace = FontFamily(
        Font(R.font.noto_sans_mono_regular, FontWeight.Normal),
        Font(R.font.noto_sans_mono_medium, FontWeight.Medium),
        Font(R.font.noto_sans_mono_medium, FontWeight.SemiBold),
        Font(R.font.noto_sans_mono_bold, FontWeight.Bold),
    )
}

// `fontFeatureSettings = "tnum"` forces tabular (fixed-width) figures on the styles that render
// big numbers — stat values, hero figures, counters. Without it, proportional digits shift the
// layout horizontally as values change or animate (e.g. a count-up from 9 → 10). Prose body styles
// keep proportional figures; label styles are already Monospace, so tabular comes for free there.
private const val TabularFigures = "tnum"

/**
 * §6 mono — the SECTION-ANCHOR rung (15sp), one step above `labelLarge`'s 13sp.
 *
 * Added 2026-07-25. The row labels moved up to 13sp, which put them on the anchor's own rung, and a
 * section anchor is usually a SHORT word ("BODY") with wide tracking against long rows ("BODY FAT"):
 * equal type size, less visual mass, so the header read as the smallest thing in its own section.
 * Measured on device — all four were an identical 26px cap height — which is exactly why matching
 * sizes was not enough. Owned by `EditorialHeader`; nothing else should reach for it.
 */
val MonoSectionAnchor = TextStyle(
    fontFamily = AvexFonts.Monospace,
    fontWeight = FontWeight.Medium,
    fontSize = 15.sp,
    lineHeight = 18.sp,
    letterSpacing = 1.sp
)

val ForgeTypography = Typography(
    displayLarge = TextStyle(
        fontFamily = AvexFonts.Serif,
        fontWeight = FontWeight.Normal,
        fontSize = 52.sp,
        lineHeight = 58.sp,
        letterSpacing = 0.sp,
        fontFeatureSettings = TabularFigures
    ),
    headlineLarge = TextStyle(
        fontFamily = AvexFonts.Serif,
        fontWeight = FontWeight.Normal,
        fontSize = 36.sp,
        lineHeight = 42.sp,
        letterSpacing = 0.sp,
        fontFeatureSettings = TabularFigures
    ),
    headlineMedium = TextStyle(
        fontFamily = AvexFonts.Serif,
        fontWeight = FontWeight.Normal,
        fontSize = 28.sp,
        lineHeight = 34.sp,
        letterSpacing = 0.sp,
        fontFeatureSettings = TabularFigures
    ),
    headlineSmall = TextStyle(
        fontFamily = AvexFonts.Serif,
        fontWeight = FontWeight.Normal,
        fontSize = 22.sp,
        lineHeight = 28.sp,
        letterSpacing = 0.sp,
        fontFeatureSettings = TabularFigures
    ),
    titleLarge = TextStyle(
        fontFamily = AvexFonts.SansSerif,
        fontWeight = FontWeight.Medium,
        fontSize = 18.sp,
        lineHeight = 24.sp,
        letterSpacing = 0.sp,
        fontFeatureSettings = TabularFigures
    ),
    titleMedium = TextStyle(
        fontFamily = AvexFonts.SansSerif,
        fontWeight = FontWeight.Medium,
        fontSize = 16.sp,
        lineHeight = 22.sp,
        letterSpacing = 0.15.sp
    ),
    titleSmall = TextStyle(
        fontFamily = AvexFonts.SansSerif,
        fontWeight = FontWeight.Medium,
        fontSize = 14.sp,
        lineHeight = 20.sp,
        letterSpacing = 0.1.sp
    ),
    bodyLarge = TextStyle(
        fontFamily = AvexFonts.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 16.sp,
        lineHeight = 24.sp,
        letterSpacing = 0.sp
    ),
    bodyMedium = TextStyle(
        fontFamily = AvexFonts.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 14.sp,
        lineHeight = 20.sp,
        letterSpacing = 0.sp
    ),
    bodySmall = TextStyle(
        fontFamily = AvexFonts.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 12.sp,
        lineHeight = 16.sp,
        letterSpacing = 0.sp
    ),
    labelLarge = TextStyle(
        fontFamily = AvexFonts.Monospace,
        fontWeight = FontWeight.Medium,
        fontSize = 13.sp,
        lineHeight = 16.sp,
        letterSpacing = 0.8.sp
    ),
    labelMedium = TextStyle(
        fontFamily = AvexFonts.Monospace,
        fontWeight = FontWeight.Normal,
        fontSize = 11.sp,
        lineHeight = 14.sp,
        letterSpacing = 0.6.sp
    ),
    labelSmall = TextStyle(
        fontFamily = AvexFonts.Monospace,
        fontWeight = FontWeight.Normal,
        fontSize = 10.sp,
        lineHeight = 12.sp,
        letterSpacing = 0.5.sp
    )
)
