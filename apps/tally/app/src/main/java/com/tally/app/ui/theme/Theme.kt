package com.tally.app.ui.theme

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance

/** WCAG contrast between two opaque colours. */
fun contrastRatio(a: Color, b: Color): Double {
    val la = a.luminance().toDouble()
    val lb = b.luminance().toDouble()
    return (maxOf(la, lb) + 0.05) / (minOf(la, lb) + 0.05)
}

/** Content on an accent fill: whichever of the palette's pair reads better, measured, not guessed. */
fun accentForeground(accent: Color, dark: Color, light: Color): Color {
    val pick = if (contrastRatio(dark, accent) >= contrastRatio(light, accent)) dark else light
    if (contrastRatio(pick, accent) >= 4.5) return pick
    return if (contrastRatio(Color.Black, accent) >= contrastRatio(Color.White, accent)) Color.Black else Color.White
}

@Composable
fun TallyTheme(
    accent: Color = Color(0xFFD4761F),
    accentEnabled: Boolean = true,
    amoled: Boolean = false,
    content: @Composable () -> Unit,
) {
    // Accent off: a near-white neutral, so selection and the primary act still read, without colour.
    val effectiveAccent = if (accentEnabled) accent else PearlOnBg
    // Remembered: ColorScheme compares by identity, and a scheme rebuilt every pass would
    // invalidate every reader of MaterialTheme.colorScheme with nothing changed.
    val scheme = remember(effectiveAccent, amoled) { tallyColorScheme(effectiveAccent, amoled) }
    MaterialTheme(colorScheme = scheme, typography = TallyTypography, shapes = TallyShapes) {
        // The ground is one flat colour, the same the window paints before Compose draws.
        Box(Modifier.fillMaxSize().background(scheme.background)) { content() }
    }
}

fun tallyColorScheme(accent: Color, amoled: Boolean): ColorScheme {
    val bg = if (amoled) Color.Black else PearlBackground
    val surface = if (amoled) Color(0xFF080808) else PearlSurface
    val surfaceVar = if (amoled) Color(0xFF111111) else PearlSurfaceVar
    return darkColorScheme(
        background = bg,
        onBackground = PearlOnBg,
        surface = surface,
        onSurface = PearlOnBg,
        surfaceVariant = surfaceVar,
        onSurfaceVariant = PearlMuted,
        outline = PearlOutline,
        outlineVariant = if (amoled) Color(0xFF1E1E1E) else Color(0xFF2A241F),
        // The container ladder walks the same warm line, so sheets, menus and pickers land on Pearl
        // instead of Material's stock purple-grey.
        surfaceContainerLowest = if (amoled) Color.Black else Color(0xFF0C0A08),
        surfaceContainerLow = if (amoled) Color(0xFF060606) else Color(0xFF16120F),
        surfaceContainer = if (amoled) Color(0xFF0A0A0A) else Color(0xFF1A1613),
        surfaceContainerHigh = if (amoled) Color(0xFF111111) else Color(0xFF221C16),
        surfaceContainerHighest = if (amoled) Color(0xFF1A1A1A) else Color(0xFF2A231C),
        // Tonal elevation is the ladder above, never an accent wash over every sheet.
        surfaceTint = surface,
        primary = accent,
        onPrimary = accentForeground(accent, dark = bg, light = PearlOnBg),
        primaryContainer = accent.copy(alpha = 0.15f),
        onPrimaryContainer = PearlOnBg,
        secondary = accent.copy(alpha = 0.6f),
        onSecondary = PearlOnBg,
        tertiary = TallySuccess,
        error = TallyError,
        errorContainer = TallyError.copy(alpha = 0.15f),
        onError = PearlBackground,
        scrim = Color.Black.copy(alpha = 0.6f),
    )
}
