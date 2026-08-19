package com.forge.wear.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import com.forge.shared.protocol.ConfigDto

/**
 * The phone doctrine translated to the wrist (DESIGN.md §16): pure black ground, the user's
 * single accent at the 1.0/0.6/0.15 ladder (delivered over /config; monochrome maps to
 * near-white), ONE serif figure per screen, mono uppercase micro-labels.
 */
data class WearColors(
    val accent: Color,
    val accentDim: Color,
    val accentWash: Color,
    val onBg: Color = Color(0xFFF2EFEA),
    val muted: Color = Color(0xFFBFB6AA),
    val outline: Color = Color(0xFF38302A),
    /** Reserved PR gold — the flash only, exactly like the phone. */
    val prGold: Color = Color(0xFFE3B341)
)

val LocalWearColors = staticCompositionLocalOf { wearColors(ConfigDto()) }

fun wearColors(config: ConfigDto): WearColors {
    val accent = if (!config.accentEnabled) Color(0xFFF2EFEA)
    else parseHex(config.accentHex) ?: Color(0xFFD4761F) // Ember default, like the phone.
    return WearColors(
        accent = accent,
        accentDim = accent.copy(alpha = 0.6f),
        accentWash = accent.copy(alpha = 0.15f)
    )
}

private fun parseHex(hex: String): Color? {
    val h = hex.removePrefix("#")
    if (h.length != 6) return null
    return h.toLongOrNull(16)?.let { Color(0xFF000000 or it) }
}

/** The three voices at wrist scale: one serif figure, quiet sans, mono micro-labels. */
object WearType {
    val figure = TextStyle(
        fontFamily = FontFamily.Serif,
        fontWeight = FontWeight.Normal,
        fontSize = 40.sp,
        letterSpacing = 0.sp
    )
    val figureSmall = figure.copy(fontSize = 26.sp)
    val body = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = 14.sp)
    /** Control glyphs (the ± steppers) — sans, a step up from body so the mark reads in a circle. */
    val control = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 16.sp,
        fontWeight = FontWeight.Medium
    )
    val label = TextStyle(
        fontFamily = FontFamily.Monospace,
        fontSize = 10.sp,
        letterSpacing = 1.2.sp
    )
    val labelSmall = label.copy(fontSize = 8.sp, letterSpacing = 1.sp)
}
