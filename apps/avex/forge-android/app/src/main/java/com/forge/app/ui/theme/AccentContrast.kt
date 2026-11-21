package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance

/** WCAG AA for normal text. Filled accent controls carry labels, so this is the bar they answer to. */
const val AA_CONTRAST = 4.5

/** WCAG contrast ratio between two opaque colours, lighter over darker. */
fun contrastRatio(a: Color, b: Color): Double {
    val la = a.luminance().toDouble()
    val lb = b.luminance().toDouble()
    val lighter = maxOf(la, lb)
    val darker = minOf(la, lb)
    return (lighter + 0.05) / (darker + 0.05)
}

/**
 * The content colour for a control FILLED with [accent] (M-37).
 *
 * The choice used to be a hard luminance threshold — dark content above 0.18, near-white below —
 * and a threshold cannot know whether the colour it picked actually reads. The theme's own pair is
 * a warm near-black and a warm near-white, neither of which is an extreme, so there is a band of
 * accents (roughly 0.15 to 0.20 relative luminance, which the custom picker accepts) where BOTH
 * fall short of AA: `#777777` measured about 4.27:1 against the dark one, and 3.9:1 against the
 * light one, on a filled capsule carrying a label.
 *
 * So the pair is chosen by measurement rather than by a threshold, and only where neither of them
 * clears AA does this reach past the palette to the pure extreme that does. Pure black and pure
 * white bracket every mid-tone: at the luminance where they are equally good (~0.179) both still
 * measure about 4.58:1, so the fallback can never fail to find one. Every curated preset and every
 * accent that already worked keeps exactly the content colour it had — the first branch is the
 * same answer the threshold gave for them, arrived at honestly.
 */
fun accentForeground(accent: Color, dark: Color, light: Color): Color {
    val fromPalette = if (contrastRatio(dark, accent) >= contrastRatio(light, accent)) dark else light
    if (contrastRatio(fromPalette, accent) >= AA_CONTRAST) return fromPalette
    return if (contrastRatio(Color.Black, accent) >= contrastRatio(Color.White, accent)) Color.Black
    else Color.White
}
