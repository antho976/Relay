package com.forge.app.ui.common

import androidx.compose.ui.graphics.Color

/** Stored accent hex ("#E85D4A") → [Color]; grey on a malformed value so one bad row can't crash a
 *  screen. Shared by the program screen, its day editor, and the onboarding week preview. */
fun parseAccentHex(hex: String): Color =
    runCatching { Color(android.graphics.Color.parseColor(hex)) }.getOrDefault(Color.Gray)
