package com.tally.app.ui.common

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color
import com.tally.app.ui.theme.Tally

/** How a reading line reads: quiet, at the limit (amber), or past it (red). */
enum class LineTone { QUIET, WARN, HELD }

/** One generated line and its tone, built where it is tested and coloured where it is drawn. */
@Immutable
data class ToneLine(val text: String, val tone: LineTone = LineTone.QUIET)

/** A line's tone as Relay's ink: ink-3 when quiet, amber at the limit, red's text colour past it. */
@Composable
fun toneColor(tone: LineTone): Color = when (tone) {
    LineTone.QUIET -> Tally.colors.ink3
    LineTone.WARN -> Tally.colors.waiting
    LineTone.HELD -> Tally.colors.heldText
}
