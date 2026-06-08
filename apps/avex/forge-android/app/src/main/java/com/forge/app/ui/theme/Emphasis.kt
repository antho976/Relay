package com.forge.app.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.font.FontWeight

/**
 * Accent-emphasis intensity (a user setting, asked at onboarding). Routes "important" text — big
 * numbers, screen/section titles, primary names — toward the theme accent and, higher up, a heavier
 * weight. OFF leaves the app's normal styling untouched: every call site reads identically because
 * [emphasized] returns the passed-in default and [emphasizedWeight] returns the passed-in default.
 *
 * Usage at an important text call site:
 * ```
 * Text(text, color = emphasized(onBg), fontWeight = emphasizedWeight())
 * ```
 */
enum class AccentEmphasis {
    OFF, SUBTLE, MEDIUM, STRONG;

    companion object {
        fun from(value: String?): AccentEmphasis =
            entries.firstOrNull { it.name.equals(value, ignoreCase = true) } ?: OFF

        /** Onboarding/settings options, strongest-first display order is up to the caller. */
        val options: List<Pair<String, String>> = listOf(
            "off" to "Off",
            "subtle" to "Subtle",
            "medium" to "Medium",
            "strong" to "Strong"
        )
    }
}

/** The current emphasis level, read from [LocalForgeSettings]. */
val currentEmphasis: AccentEmphasis
    @Composable @ReadOnlyComposable
    get() = AccentEmphasis.from(LocalForgeSettings.current.accentEmphasis)

/**
 * Recolor an "important" text element toward the accent based on the active emphasis level. Pass the
 * element's normal color as [default]; it is returned unchanged when emphasis is OFF.
 */
@Composable
@ReadOnlyComposable
fun emphasized(default: Color): Color {
    val accent = MaterialTheme.colorScheme.primary
    return when (currentEmphasis) {
        AccentEmphasis.OFF -> default
        AccentEmphasis.SUBTLE -> lerp(default, accent, 0.45f)
        AccentEmphasis.MEDIUM -> accent
        AccentEmphasis.STRONG -> accent
    }
}

/**
 * A heavier font weight for important text at higher emphasis. Pass the element's normal weight as
 * [default] (null = inherit from the text style); returned unchanged for OFF/SUBTLE.
 */
@Composable
@ReadOnlyComposable
fun emphasizedWeight(default: FontWeight? = null): FontWeight? =
    when (currentEmphasis) {
        AccentEmphasis.OFF, AccentEmphasis.SUBTLE -> default
        AccentEmphasis.MEDIUM -> FontWeight.SemiBold
        AccentEmphasis.STRONG -> FontWeight.Bold
    }
