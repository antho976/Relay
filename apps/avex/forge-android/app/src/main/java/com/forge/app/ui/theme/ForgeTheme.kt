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

@Composable
fun ForgeTheme(
    amoledMode: Boolean = false,
    accentColorHex: String = "",
    content: @Composable () -> Unit
) {
    val accent = remember(accentColorHex) {
        accentColorHex.takeIf { it.isNotEmpty() }
            ?.let { runCatching { Color(android.graphics.Color.parseColor(it)) }.getOrNull() }
            ?: AccentNavy
    }

    val scheme = pearlColorScheme(accent, amoledMode)

    val gradTop    = if (amoledMode) Color.Black         else PearlGradTop
    val gradBottom = if (amoledMode) Color(0xFF050507)   else PearlGradBottom

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
    return darkColorScheme(
        background        = bg,
        onBackground      = PearlOnBg,
        surface           = surface,
        onSurface         = PearlOnBg,
        surfaceVariant    = surfaceVar,
        onSurfaceVariant  = PearlMuted,
        outline           = PearlOutline,
        primary           = PearlOnBg,
        onPrimary         = bg,
        primaryContainer  = surfaceVar,
        onPrimaryContainer = PearlOnBg,
        secondary         = accent,
        onSecondary       = PearlOnBg,
        tertiary          = ForgeSuccess,
        error             = ForgeError,
        errorContainer    = ForgeError.copy(alpha = 0.15f),
        onError           = PearlOnBg
    )
}
