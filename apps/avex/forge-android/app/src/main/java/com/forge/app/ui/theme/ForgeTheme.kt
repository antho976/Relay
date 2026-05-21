package com.forge.app.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable

private val ForgeColorScheme = darkColorScheme(
    background = ForgeBackground,
    onBackground = ForgeOnBackground,
    surface = ForgeSurface,
    onSurface = ForgeOnBackground,
    surfaceVariant = ForgeSurfaceElevated,
    onSurfaceVariant = ForgeOnSurfaceMuted,
    outline = ForgeOutline,
    primary = ForgeAccent,
    onPrimary = ForgeOnBackground,
    secondary = ForgeAccentMuted,
    error = ForgeAccent,
    tertiary = ForgeSuccess
)

@Composable
fun ForgeTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = ForgeColorScheme,
        typography = ForgeTypography,
        shapes = ForgeShapes,
        content = content
    )
}
