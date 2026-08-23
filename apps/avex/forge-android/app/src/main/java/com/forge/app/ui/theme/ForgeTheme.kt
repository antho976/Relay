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
            ?: AccentRed
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
    // M3's container family, themed (2026-08-20). Left unset, every component that fills itself —
    // ModalBottomSheet (surfaceContainerLow), DatePickerDialog + AlertDialog (surfaceContainerHigh),
    // DropdownMenu (surfaceContainer), and their dividers (outlineVariant) — fell through to
    // Material's STOCK dark palette: a lighter, purple-leaning grey belonging to no theme here.
    // Found on device as a pale slab on near-black (`design/AUDIT.md`, 2026-07-25); until now every
    // call site paid for it one containerColor at a time, and eleven of them in settings never did.
    // The ladder walks the same warm Pearl line the rest of §5 does, bg → surface → surfaceVariant.
    val cLowest    = if (amoled) Color.Black         else Color(0xFF0C0A08)
    val cLow       = if (amoled) Color(0xFF060606)   else Color(0xFF16120F)
    val cBase      = if (amoled) Color(0xFF0A0A0A)   else Color(0xFF1A1613)
    val cHigh      = if (amoled) Color(0xFF111111)   else Color(0xFF221C16)
    val cHighest   = if (amoled) Color(0xFF1A1A1A)   else Color(0xFF2A231C)
    // The quiet rung under `outline` — M3 uses it for the rules inside its own components (the
    // date-picker header, menu separators). Data lines still come from EditorialHairline (§1).
    val outlineVar = if (amoled) Color(0xFF1E1E1E)   else Color(0xFF2A241F)
    // Content ON an accent fill: dark for a light accent, else the near-white default — so a
    // filled-primary control never renders same-on-same.
    //
    // The threshold is 0.18, not 0.55 (2026-08-16). The default Red sits at luminance 0.198 and Ember
    // at 0.271: near-white on either FAILS AA, while dark-on-them measures 4.53:1 and 5.84:1 and
    // passes. 0.55 only ever caught a near-white accent; anything genuinely mid-tone — which the warm
    // presets are — needs dark content. The dim presets (Navy 0.077, Gold 0.135) keep near-white.
    // Note how close the default now sits to the threshold: an accent darker than 0.18 flips back to
    // near-white content, so do not nudge the default down without re-checking both sides.
    val onAccent   = if (accent.luminance() > 0.18f) bg else PearlOnBg
    return darkColorScheme(
        background         = bg,
        onBackground       = PearlOnBg,
        surface            = surface,
        onSurface          = PearlOnBg,
        surfaceVariant     = surfaceVar,
        onSurfaceVariant   = PearlMuted,
        outline            = PearlOutline,
        outlineVariant     = outlineVar,
        surfaceContainerLowest  = cLowest,
        surfaceContainerLow     = cLow,
        surfaceContainer        = cBase,
        surfaceContainerHigh    = cHigh,
        surfaceContainerHighest = cHighest,
        // Tonal elevation is the surface ladder above, not an accent wash: M3 blends `surfaceTint`
        // into a raised surface, so leaving it at the default tinted every sheet and menu with the
        // user's accent. Pointing it at `surface` makes that blend a no-op and keeps §5's rule that
        // colour is scarce and always means something.
        surfaceTint        = surface,
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
