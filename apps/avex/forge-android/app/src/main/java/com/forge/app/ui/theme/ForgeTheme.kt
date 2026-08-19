package com.forge.app.ui.theme

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance

@Composable
fun ForgeTheme(
    amoledMode: Boolean = false,
    accentColorHex: String = "",
    accentEnabled: Boolean = true,
    content: @Composable () -> Unit
) {
    val accent = remember(accentColorHex, accentEnabled) {
        // Accent off ⇒ a near-white neutral so highlights (selected pill, active nav, chart strokes)
        // stay legible and distinct from the muted rest, just without any colour.
        if (!accentEnabled) PearlOnBg
        else accentColorHex.takeIf { it.isNotEmpty() }
            ?.let { runCatching { Color(android.graphics.Color.parseColor(it)) }.getOrNull() }
            ?: AccentEmber
    }

    val scheme = pearlColorScheme(accent, amoledMode)

    val (gradTop, gradBottom) = forgeBackgroundGradient(amoledMode)

    MaterialTheme(
        colorScheme = scheme,
        typography  = ForgeTypography,
        shapes      = ForgeShapes
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(Brush.verticalGradient(listOf(gradTop, gradBottom)))
        ) {
            content()
        }
    }
}

/**
 * The vertical background gradient (top → bottom) the app paints behind every screen, keyed only on
 * [amoled] (the accent does not affect it). Single source of truth so the Compose theme AND the
 * pre-content window background ([MainActivity] sets a matching drawable at boot) stay in lock-step —
 * change the colors here and the cold-start background follows automatically.
 */
fun forgeBackgroundGradient(amoled: Boolean): Pair<Color, Color> =
    if (amoled) Color.Black to Color(0xFF050507)
    else PearlGradTop to PearlGradBottom

// Indigo light scheme — available for future light-theme support
@Suppress("unused")
fun indigoColorScheme(accent: Color = AccentIndigoDefault): ColorScheme =
    lightColorScheme(
        background        = IndigoBackground,
        onBackground      = IndigoOnBg,
        surface           = IndigoSurface,
        onSurface         = IndigoOnBg,
        surfaceVariant    = IndigoSurfaceVar,
        onSurfaceVariant  = IndigoMuted,
        outline           = IndigoOutline,
        primary           = IndigoOnBg,
        onPrimary         = IndigoBackground,
        primaryContainer  = IndigoSurfaceVar,
        onPrimaryContainer = IndigoOnBg,
        secondary         = accent,
        onSecondary       = IndigoOnBg,
        tertiary          = ForgeSuccess,
        error             = ForgeError
    )

private fun pearlColorScheme(accent: Color, amoled: Boolean): ColorScheme {
    val bg         = if (amoled) Color.Black         else PearlBackground
    val surface    = if (amoled) Color(0xFF080808)   else PearlSurface
    val surfaceVar = if (amoled) Color(0xFF111111)   else PearlSurfaceVar
    // Content ON an accent fill: dark for a light accent, else the near-white default — so a
    // filled-primary control never renders same-on-same.
    //
    // The threshold is 0.18, not 0.55 (2026-08-16). Ember sits at luminance 0.271: near-white on it
    // measures 3.44:1 and FAILS AA, while dark-on-ember measures 5.84:1 and passes. 0.55 only ever
    // caught a near-white accent; anything genuinely mid-tone — which the warm presets are — needs
    // dark content. The dim presets (Navy 0.077, Gold 0.135) are unaffected and keep near-white.
    val onAccent   = if (accent.luminance() > 0.18f) bg else PearlOnBg
    return darkColorScheme(
        background         = bg,
        onBackground       = PearlOnBg,
        surface            = surface,
        onSurface          = PearlOnBg,
        surfaceVariant     = surfaceVar,
        onSurfaceVariant   = PearlMuted,
        outline            = PearlOutline,
        primary            = accent,                          // user-editable brand/accent
        onPrimary          = onAccent,
        primaryContainer   = accent.copy(alpha = 0.15f),
        onPrimaryContainer = PearlOnBg,
        secondary          = accent.copy(alpha = 0.6f),
        onSecondary        = PearlOnBg,
        tertiary           = ForgeSuccess,
        error              = ForgeError,
        errorContainer     = ForgeError.copy(alpha = 0.15f),
        onError            = PearlOnBg
    )
}
