package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Color

/**
 * Parses a "#RRGGBB" or "#AARRGGBB" hex string into a Compose [Color]. Throws on bad
 * input — these are static program constants, so a malformed colour is a developer
 * bug to fix in source, not a runtime user input to handle gracefully.
 */
fun String.toAccentColor(): Color {
    val hex = removePrefix("#")
    val value = hex.toLong(16)
    return when (hex.length) {
        6 -> Color(value or 0xFF000000)
        8 -> Color(value)
        else -> error("Bad hex color: '$this' — expected #RRGGBB or #AARRGGBB")
    }
}
