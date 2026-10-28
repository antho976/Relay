package com.quietsoftware.relay.ui.theme

import androidx.compose.foundation.text.selection.LocalTextSelectionColors
import androidx.compose.foundation.text.selection.TextSelectionColors
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import com.quietsoftware.relay.R

/**
 * The app's colours: a background family (the ground, cards, sheets and code wells) and an accent
 * (the one primary key per surface, the send key, switches and selections), both chosen in
 * Settings › Appearance. Words and state colours (live green, needs-you amber, refused red) are
 * the same in every theme, so a lamp reads the same whatever the person picked.
 */
@Immutable
data class Palette(
    val name: String,
    /** App ground, bars, the drawer. */
    val wall: Color,
    val console: Color,
    /** Keys, cards, pills, popovers. */
    val slab: Color,
    /** Pressed and selected rows, the person's own bubble. */
    val wash: Color,
    /** Terminals and code only. */
    val screen: Color,
    val ink: Color,
    val ink2: Color,
    val edge: Color,
    val ink3: Color,
    val strong: Color,
    /** The drawer and sheets: a step under the ground. */
    val pane: Color = screen,
    val raised: Color = slab,
    val track: Color = wash,
    val lineSubtle: Color = edge,
    val lineEmphasis: Color = strong,
    val lineFocus: Color = strong,
    val accent: Color = Accent.Clay.color,
) {
    val inkDim = Color(0xFF6E6A64)
    val live = Color(0xFF2EC469)
    val held = Color(0xFFE5382E)
    val waiting = Color(0xFFF0A828)
    /** Red for words: a refusal's reason, a held name. */
    val heldText = Color(0xFFF0786F)
    val selection = accent.copy(alpha = .30f)
    /** Words on the accent: every accent is light enough to carry the ground's near-black. */
    val onAccent = Color(0xFF1F1E1C)
    /** The accent as a tint behind a selected row or a soft key. */
    val accentSoft = accent.copy(alpha = .17f)
    /** The accent dimmed for words on the ground (a link, a count). */
    val accentText = lerp(accent, Color.White, .25f)
    /** The terminal's own colours (terminal.rs). */
    val termFg = Color(0xFFDCDCDA)
    val termCursor = Color(0xFFECECEA)

    companion object {
        /** The background families, in the order Settings shows them. */
        val BASES = listOf(
            Base("grey", "Grey", Color(0xFF262624), Color(0xFF30302E), Color(0xFF3A3936), Color(0xFF1D1C1B)),
            Base("slate", "Slate", Color(0xFF1F2328), Color(0xFF2A2F36), Color(0xFF343A42), Color(0xFF181B1F)),
            Base("midnight", "Midnight", Color(0xFF141B2D), Color(0xFF1E2740), Color(0xFF283350), Color(0xFF0F1524)),
            Base("forest", "Forest", Color(0xFF18211D), Color(0xFF222D28), Color(0xFF2C3933), Color(0xFF121915)),
            Base("plum", "Plum", Color(0xFF221A26), Color(0xFF2D2332), Color(0xFF382D3E), Color(0xFF1A141D)),
            Base("black", "Black", Color(0xFF000000), Color(0xFF141414), Color(0xFF1E1E1E), Color(0xFF0A0A0A)),
        )

        /** The ground a name stands for; the PC's older names (matte, dark, oled) keep working. */
        fun base(name: String): Base = when (name) {
            "oled" -> BASES.last()
            "matte", "dark" -> BASES.first()
            else -> BASES.firstOrNull { it.key == name } ?: BASES.first()
        }

        fun of(name: String, accent: String = Accent.Clay.key): Palette = base(name).palette(Accent.of(accent))

        val Matte = of("grey")

        /** ANSI 0–15 as the PC's terminals draw them (terminal.rs). */
        val ANSI = intArrayOf(
            0x1A1A1D, 0xE5382E, 0x3FBF74, 0xE0B04A, 0x5B9CF6, 0xC979D6, 0x3EC5CF, 0xC8C8C6,
            0x5C5C60, 0xFF6B5F, 0x5FE08C, 0xF2CF6B, 0x8AB8FF, 0xE19BEA, 0x68DFE8, 0xF2F2F0,
        )

        /** Board columns (board.css). */
        fun column(c: String, p: Palette) = when (c) {
            "ready" -> Color(0xFF4493F8)
            "active" -> p.waiting
            "in_review" -> Color(0xFFAB7DF8)
            "done" -> p.live
            else -> p.ink3
        }

        /** Task label hues (board.css). */
        val LABELS = listOf(0xFF8FA3BF, 0xFFA597C4, 0xFFC4949F, 0xFFC4AA8B, 0xFF8FB89C, 0xFF8DB6BD, 0xFFB3B78B, 0xFFC09A8D).map { Color(it) }

        /** Tally's category hues (money_pages.rs). */
        val HUES = listOf(0xFF7FB27A, 0xFF4FA9A0, 0xFF6A9FD8, 0xFF8C87D9, 0xFFC27BC0, 0xFFD9768E, 0xFFE08A5F, 0xFFD9A441, 0xFFA3A84E, 0xFFC2A585, 0xFF8D99A6, 0xFFB8664F).map { Color(it) }

        /** Git status letters (git_files.css). */
        val GIT_MODIFIED = Color(0xFFE2C08D)
        val GIT_ADDED = Color(0xFF73C991)
        val GIT_DELETED = Color(0xFFE5675B)
        val GIT_RENAMED = Color(0xFF73B2E8)
        val SHA = Color(0xFFC9A24C)

        /** The start screen's own colours (start.rs). */
        val START_GROUND = Color(0xFF0F1114)
        val START_CARD = Color(0xFF16181C)
        val START_EDGE = Color(0xFF23262C)
        val START_DIM = Color(0xFF63666B)
        val START_TEXT2 = Color(0xFF8B8F96)
        val SIGNAL = Color(0xFFC6F24E)
    }
}

/** A background family: its ground, cards, raised cards and the well under them. */
@Immutable
data class Base(val key: String, val label: String, val ground: Color, val card: Color, val card2: Color, val deep: Color) {
    fun palette(accent: Accent): Palette = Palette(
        name = key,
        wall = ground,
        console = ground,
        slab = card,
        wash = card2,
        screen = if (ground == Color.Black) Color.Black else lerp(deep, Color.Black, .25f),
        ink = Color(0xFFF5F4EF),
        ink2 = Color(0xFFB9B4AA),
        edge = lerp(ground, Color.White, .08f),
        ink3 = Color(0xFF9C978D),
        strong = lerp(ground, Color.White, .13f),
        pane = deep,
        raised = card,
        track = card2,
        lineSubtle = lerp(ground, Color.White, .05f),
        lineEmphasis = lerp(ground, Color.White, .12f),
        lineFocus = lerp(ground, Color.White, .17f),
        accent = accent.color,
    )
}

/** The accents Settings offers. Each is light enough for [Palette.onAccent] to read on it. */
enum class Accent(val key: String, val label: String, val color: Color) {
    Clay("clay", "Clay", Color(0xFFD97757)),
    Ocean("ocean", "Ocean", Color(0xFF5B9CF6)),
    Sage("sage", "Sage", Color(0xFF7FB685)),
    Iris("iris", "Iris", Color(0xFF9B8AFB)),
    Rose("rose", "Rose", Color(0xFFE8789A)),
    Amber("amber", "Amber", Color(0xFFE5B567)),
    Mint("mint", "Mint", Color(0xFF4FC6B8));

    companion object {
        fun of(key: String) = entries.firstOrNull { it.key == key } ?: Clay
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
    val FiraMono = FontFamily(
        Font(R.font.fira_mono_400, FontWeight.Normal),
        Font(R.font.fira_mono_500, FontWeight.Medium),
    )
    val FiraSans = FontFamily(
        Font(R.font.fira_sans_500, FontWeight.Medium),
        Font(R.font.fira_sans_600, FontWeight.SemiBold),
    )
    val FiraCondensed = FontFamily(
        Font(R.font.fira_sans_condensed_500, FontWeight.Medium),
        Font(R.font.fira_sans_condensed_600, FontWeight.SemiBold),
    )
}

/**
 * The PC's type scale, a step larger for a phone held at arm's length: 13px controls become 14sp,
 * 12px captions 12.5sp, 11.5px figures 12sp. Weights and families are the PC's.
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
    /** Small capitals over a group: 11/600 with 0.08em tracking. */
    val eyebrow: TextStyle = TextStyle(fontFamily = Fonts.Geist, fontSize = 11.5.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.08.em),
    val mono: TextStyle = TextStyle(fontFamily = Fonts.GeistMono, fontSize = 12.sp, lineHeight = 16.sp),
    val code: TextStyle = TextStyle(fontFamily = Fonts.GeistMono, fontSize = 13.sp, lineHeight = 19.sp),
    val brand: TextStyle = TextStyle(fontFamily = Fonts.Sora, fontSize = 18.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.5).sp),
    /** The terminal strip's name: Fira Sans 13/600. */
    val plateName: TextStyle = TextStyle(fontFamily = Fonts.FiraSans, fontSize = 14.sp, fontWeight = FontWeight.SemiBold),
    /** The strip's metadata: Fira Sans Condensed 11/500, upper case. */
    val plateMeta: TextStyle = TextStyle(fontFamily = Fonts.FiraCondensed, fontSize = 11.5.sp, fontWeight = FontWeight.Medium, letterSpacing = 0.04.em),
    val plateState: TextStyle = TextStyle(fontFamily = Fonts.FiraCondensed, fontSize = 11.5.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.08.em),
)

val LocalPalette = staticCompositionLocalOf { Palette.Matte }
val LocalType = staticCompositionLocalOf { Type() }

object Relay {
    val colors: Palette @Composable get() = LocalPalette.current
    val type: Type @Composable get() = LocalType.current
}

@Composable
fun RelayTheme(palette: Palette = Palette.Matte, content: @Composable () -> Unit) {
    CompositionLocalProvider(
        LocalPalette provides palette,
        LocalType provides Type(),
        LocalTextSelectionColors provides TextSelectionColors(handleColor = palette.accent, backgroundColor = palette.selection),
        content = content,
    )
}
