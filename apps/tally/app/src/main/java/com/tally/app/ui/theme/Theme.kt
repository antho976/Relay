package com.tally.app.ui.theme

import androidx.compose.foundation.text.selection.LocalTextSelectionColors
import androidx.compose.foundation.text.selection.TextSelectionColors
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.foundation.background
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
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
 * Direction C, "Postcard sky" (DESIGN.md): a sky gradient for the ground, frosted white glass cards,
 * deep ink, and colour that means something: categories, money in, pace. The names stay Relay's so
 * every component reads the same tokens; four grounds share them (Sky, Dusk, Mint morning, Night).
 */
@Immutable
data class Palette(
    val name: String,
    /** A solid stand-in for the ground: bars that must be opaque, text on an ink key. */
    val wall: Color,
    /** Frosted bars: the bottom navigation, an editor's footer. */
    val console: Color,
    /** Glass: cards, keys, pills. */
    val slab: Color,
    /** Pressed and selected rows. */
    val wash: Color,
    /** The deepest plate: a switch's track. */
    val screen: Color,
    val ink: Color,
    val ink2: Color,
    val edge: Color,
    val ink3: Color,
    val strong: Color,
    /** The sky, top to bottom. */
    val ground: List<Color>,
    val light: Boolean = true,
    /** Fields and sheets: solid, so typing never sits on glass. */
    val raised: Color = Color.White,
    val track: Color = Color(0x1A1A1A2E),
    val lineSubtle: Color = Color(0x141A1A2E),
    val lineFocus: Color = Color(0xFF6C4BF4),
    /** On pace, money in, synced. */
    val live: Color = Color(0xFF13A36E),
    /** Over budget, a refusal. As a mark only; words in red use [heldText]. */
    val held: Color = Color(0xFFE5382E),
    /** Over pace, waiting. */
    val waiting: Color = Color(0xFFF0A828),
    val heldText: Color = Color(0xFFB3261E),
    val liveText: Color = Color(0xFF0A7A50),
    val waitingText: Color = Color(0xFF7A4A00),
    /** The one accent: links, the focused field, the mark's slash. */
    val signal: Color = Color(0xFF6C4BF4),
    /** Text on an ink key. */
    val onInk: Color = Color.White,
) {
    val selection = ink.copy(alpha = .22f)
    val groundBrush: Brush get() = Brush.verticalGradient(ground)

    companion object {
        private val LightInk = Color(0xFF1A1A2E)
        private val LightInk2 = Color(0xFF4A4A63)
        private val LightInk3 = Color(0xFF626279)

        val Sky = Palette(
            "sky", wall = Color(0xFFEFEEFB), console = Color(0xF2FFFFFF), slab = Color(0xBFFFFFFF),
            wash = Color(0x141A1A2E), screen = Color(0x991A1A2E).copy(alpha = .06f),
            ink = LightInk, ink2 = LightInk2, edge = Color(0x00000000), ink3 = LightInk3, strong = Color(0x261A1A2E),
            ground = listOf(Color(0xFFA9D8FF), Color(0xFFE6E4FF), Color(0xFFFFE4D4)),
        )
        val Dusk = Sky.copy(
            name = "dusk", wall = Color(0xFFF3ECFB),
            ground = listOf(Color(0xFFC9B8FF), Color(0xFFF2C9E0), Color(0xFFFFD9BC)),
        )
        val Mint = Sky.copy(
            name = "mint", wall = Color(0xFFEFF8F2),
            ground = listOf(Color(0xFFB9EDD6), Color(0xFFE6F6EE), Color(0xFFFFF2DE)),
        )
        val Night = Palette(
            "night", wall = Color(0xFF15173A), console = Color(0xF21A1C40), slab = Color(0x1FFFFFFF),
            wash = Color(0x1FFFFFFF), screen = Color(0x33000000),
            ink = Color(0xFFEDEBF7), ink2 = Color(0xFFBDBAD6), edge = Color(0x00000000), ink3 = Color(0xFF9A97B8), strong = Color(0x33FFFFFF),
            ground = listOf(Color(0xFF0E1330), Color(0xFF1B1640), Color(0xFF2A1838)),
            light = false, raised = Color(0xFF23264D), track = Color(0x26FFFFFF), lineSubtle = Color(0x1AFFFFFF),
            lineFocus = Color(0xFFA996FF), live = Color(0xFF2EC469), heldText = Color(0xFFF0786F), liveText = Color(0xFF5FD38E),
            waitingText = Color(0xFFF5C46A), signal = Color(0xFFA996FF), onInk = Color(0xFF15173A),
        )

        val ALL = listOf(Sky, Dusk, Mint, Night)

        /** Older names (Relay's grounds) land on the closest sky. */
        fun of(name: String?) = when (name) {
            "dusk" -> Dusk
            "mint" -> Mint
            "night", "dark", "oled" -> Night
            else -> Sky
        }

        /** The ring mark's dimmed arc. */
        val DIM = Color(0xFF9A97B8)
    }
}

/** Soft tints and strong marks for colour that means something, drawn from the C mockups. */
object Hue {
    val violet = Color(0xFF6C4BF4)
    val mint = Color(0xFF13A36E)
    val coral = Color(0xFFFF5A4E)
    val sky = Color(0xFF2F7CF6)
    val sun = Color(0xFFFFB020)
    val pink = Color(0xFFF0508C)
    val teal = Color(0xFF0EA5A0)
    val mintSoft = Color(0xFFDCF5E9)
    val peachSoft = Color(0xFFFFE5D9)
    val lilacSoft = Color(0xFFECE6FF)
    val skySoft = Color(0xFFDDEAFF)
    val sunSoft = Color(0xFFFFF0C7)
    val pinkSoft = Color(0xFFFFE0EC)
    /** Words on the soft tints, each 4.5:1 on its tint. */
    val mintInk = Color(0xFF085C3C)
    val peachInk = Color(0xFF7A3A26)
    val lilacInk = Color(0xFF3B2A86)
    val skyInk = Color(0xFF173E86)
    val sunInk = Color(0xFF5E3A00)
    val pinkInk = Color(0xFF7A1F3D)
}

/** A screen's ground: the palette's sky. */
fun Modifier.ground(p: Palette): Modifier = this.background(p.groundBrush)

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
    val title: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 17.sp, fontWeight = FontWeight.SemiBold, lineHeight = 22.sp),
    val heading: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 26.sp, fontWeight = FontWeight.SemiBold, lineHeight = 31.sp, letterSpacing = (-0.02).em),
    val nav: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 15.sp, lineHeight = 20.sp),
    val section: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, lineHeight = 21.sp),
    /** Small capitals over a group: 11.5/600 with 0.08em tracking. */
    val eyebrow: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 11.5.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.08.em),
    val mono: TextStyle = TextStyle(fontFamily = Fonts.GeistMono, fontSize = 12.sp, lineHeight = 16.sp),
    val brand: TextStyle = TextStyle(fontFamily = Fonts.Sora, fontSize = 18.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.5).sp),
    /** An amount in a row. */
    val amount: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 14.sp, fontWeight = FontWeight.Medium, lineHeight = 19.sp, fontFeatureSettings = "tnum"),
    /** A figure in a stat card. */
    val figure: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 21.sp, fontWeight = FontWeight.SemiBold, lineHeight = 26.sp, fontFeatureSettings = "tnum"),
    /** The one large figure a page opens on. */
    val hero: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 46.sp, fontWeight = FontWeight.SemiBold, lineHeight = 46.sp, letterSpacing = (-0.03).em, fontFeatureSettings = "tnum"),
)

val LocalPalette = staticCompositionLocalOf { Palette.Sky }
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
fun TallyTheme(palette: Palette = Palette.Sky, content: @Composable () -> Unit) {
    val base = if (palette.light) lightColorScheme() else darkColorScheme()
    val scheme = base.copy(
        primary = palette.ink,
        onPrimary = palette.onInk,
        secondary = palette.ink2,
        onSecondary = palette.onInk,
        background = palette.wall,
        onBackground = palette.ink,
        surface = palette.raised,
        onSurface = palette.ink,
        surfaceVariant = palette.raised,
        onSurfaceVariant = palette.ink2,
        surfaceContainer = palette.raised,
        surfaceContainerHigh = palette.raised,
        surfaceContainerHighest = palette.wash,
        surfaceContainerLow = palette.wall,
        surfaceContainerLowest = palette.wall,
        outline = palette.strong,
        outlineVariant = palette.edge,
        error = palette.held,
        onError = Color.White,
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
