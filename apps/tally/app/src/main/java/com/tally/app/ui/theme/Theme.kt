package com.tally.app.ui.theme

import androidx.compose.foundation.text.selection.LocalTextSelectionColors
import androidx.compose.foundation.text.selection.TextSelectionColors
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import com.tally.app.R

/**
 * Relay's tokens (apps/relay-android's Theme.kt, from the desktop's DESIGN.md), so Tally looks like
 * Relay's phone app: warm near-black chrome in Geist, off-white primary keys, colour only where it
 * means something (a state, or a category's hue as data).
 */
@Immutable
data class Palette(
    val name: String,
    /** App ground, bars, the drawer. */
    val wall: Color,
    /** The status bar's ground. */
    val console: Color,
    /** Keys, cards, pills. */
    val slab: Color,
    /** Pressed and selected rows. */
    val wash: Color,
    /** The deepest ground: the space switch, a plate. */
    val screen: Color,
    val ink: Color,
    val ink2: Color,
    val edge: Color,
    val ink3: Color,
    val strong: Color,
) {
    val raised = Color(0xFF1C1B19)
    val track = Color(0xFF2A2826)
    val lineSubtle = Color(0xFF211F1D)
    val lineFocus = Color(0xFF3A3733)
    /** On pace, money in, synced. */
    val live = Color(0xFF2EC469)
    /** Over budget, a refusal. As a mark only; words in red use [heldText]. */
    val held = Color(0xFFE5382E)
    /** Over pace, waiting. */
    val waiting = Color(0xFFF0A828)
    val heldText = Color(0xFFF0786F)
    val liveText = Color(0xFF5FD38E)
    val selection = ink.copy(alpha = .22f)
    /** Relay's signal green: the mark's slash, a figure that is good news. */
    val signal = Color(0xFFC6F24E)

    companion object {
        val Matte = Palette("matte", Color(0xFF171614), Color(0xFF171614), Color(0xFF1E1D1B), Color(0xFF23211F), Color(0xFF0A0A0B), Color(0xFFEDE9E2), Color(0xFFB5B0A8), Color(0xFF292725), Color(0xFF8C877F), Color(0xFF2C2A28))
        val Dark = Palette("dark", Color(0xFF131211), Color(0xFF131211), Color(0xFF1A1917), Color(0xFF1F1D1B), Color(0xFF08090A), Color(0xFFEDE9E2), Color(0xFFB5B0A8), Color(0xFF242220), Color(0xFF8C877F), Color(0xFF262422))
        val Oled = Palette("oled", Color(0xFF000000), Color(0xFF000000), Color(0xFF0F0E0D), Color(0xFF161513), Color(0xFF000000), Color(0xFFEDE9E2), Color(0xFFB5B0A8), Color(0xFF1C1B19), Color(0xFF8C877F), Color(0xFF211F1D))

        val ALL = listOf(Matte, Dark, Oled)

        fun of(name: String?) = when (name) {
            "dark" -> Dark
            "oled" -> Oled
            else -> Matte
        }

        /** The ring mark's dimmed arc and the start screen's quiet text (start.rs). */
        val DIM = Color(0xFF63666B)
    }
}

object Fonts {
    val Geist = FontFamily(
        Font(R.font.geist_400, FontWeight.Normal),
        Font(R.font.geist_500, FontWeight.Medium),
        Font(R.font.geist_600, FontWeight.SemiBold),
    )
    val GeistMono = FontFamily(
        Font(R.font.geist_mono_400, FontWeight.Normal),
        Font(R.font.geist_mono_500, FontWeight.Medium),
    )
    val Sora = FontFamily(Font(R.font.sora_600, FontWeight.SemiBold))
}

/**
 * Relay's phone type scale. Figures (amounts) take tabular numerals everywhere, so a column of
 * them lines up and a total does not jitter as it counts.
 */
@Immutable
data class Type(
    val ui: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 14.sp, lineHeight = 19.sp),
    val uiMedium: TextStyle = ui.copy(fontWeight = FontWeight.Medium),
    val body: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 15.sp, lineHeight = 22.sp),
    val caption: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 12.5.sp, lineHeight = 17.sp),
    val title: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, lineHeight = 21.sp),
    val heading: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 22.sp, fontWeight = FontWeight.SemiBold, lineHeight = 28.sp),
    val nav: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 15.sp, lineHeight = 20.sp),
    val section: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 13.sp, fontWeight = FontWeight.Medium, lineHeight = 17.sp),
    /** Small capitals over a group: 11.5/600 with 0.08em tracking. */
    val eyebrow: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 11.5.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.08.em),
    val mono: TextStyle = TextStyle(fontFamily = Fonts.GeistMono, fontSize = 12.sp, lineHeight = 16.sp),
    val brand: TextStyle = TextStyle(fontFamily = Fonts.Sora, fontSize = 18.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.5).sp),
    /** An amount in a row. */
    val amount: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 14.sp, fontWeight = FontWeight.Medium, lineHeight = 19.sp, fontFeatureSettings = "tnum"),
    /** A figure in a stat card. */
    val figure: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 21.sp, fontWeight = FontWeight.SemiBold, lineHeight = 26.sp, fontFeatureSettings = "tnum"),
    /** The one large figure a page opens on. */
    val hero: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 40.sp, fontWeight = FontWeight.SemiBold, lineHeight = 46.sp, letterSpacing = (-0.03).em, fontFeatureSettings = "tnum"),
)

val LocalPalette = staticCompositionLocalOf { Palette.Matte }
val LocalType = staticCompositionLocalOf { Type() }

object Tally {
    val colors: Palette @Composable get() = LocalPalette.current
    val type: Type @Composable get() = LocalType.current
}

/**
 * Relay's look for everything Tally draws itself; Material's few pieces Tally still uses (the date
 * picker, the drawer) get a dark scheme in the same colours so they do not stand apart.
 */
@Composable
fun TallyTheme(palette: Palette = Palette.Matte, content: @Composable () -> Unit) {
    val scheme = darkColorScheme(
        primary = palette.ink,
        onPrimary = palette.wall,
        secondary = palette.ink2,
        onSecondary = palette.wall,
        background = palette.wall,
        onBackground = palette.ink,
        surface = palette.slab,
        onSurface = palette.ink,
        surfaceVariant = palette.raised,
        onSurfaceVariant = palette.ink2,
        surfaceContainer = palette.slab,
        surfaceContainerHigh = palette.slab,
        surfaceContainerHighest = palette.wash,
        surfaceContainerLow = palette.wall,
        surfaceContainerLowest = palette.wall,
        outline = palette.strong,
        outlineVariant = palette.edge,
        error = palette.held,
        onError = palette.wall,
    )
    MaterialTheme(colorScheme = scheme) {
        CompositionLocalProvider(
            LocalPalette provides palette,
            LocalType provides Type(),
            LocalTextSelectionColors provides TextSelectionColors(handleColor = palette.ink, backgroundColor = palette.selection),
            content = content,
        )
    }
}
