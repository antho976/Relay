package com.forge.wear.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.wear.compose.material.CircularProgressIndicator
import androidx.wear.compose.material.Text

/** The wrist capsule (44dp min): surface fill earned by interactivity, sentence-case sans label. */
@Composable
fun WristCapsule(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    filled: Boolean = false
) {
    val colors = LocalWearColors.current
    Box(
        modifier = modifier
            .defaultMinSize(minHeight = 44.dp)
            .clip(RoundedCornerShape(50))
            .background(if (filled) colors.onBg else Color(0xFF15161B))
            .clickable(onClick = onClick)
            .padding(horizontal = 20.dp, vertical = 12.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = WearType.body, color = if (filled) Color.Black else colors.onBg)
    }
}

/**
 * Round ± stepper (§16: touch capsules always present beside the bezel — not every watch has a
 * rotary). 44dp touch target, 34dp visual circle so the pair fits beside the serif figure it
 * flanks — the figure IS the value being stepped, so the glyphs need no state color.
 */
@Composable
fun WristStepper(
    glyph: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val colors = LocalWearColors.current
    Box(
        modifier = modifier
            .size(44.dp)
            .clip(CircleShape)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Box(
            Modifier
                .size(34.dp)
                .clip(CircleShape)
                .background(Color(0xFF15161B)),
            contentAlignment = Alignment.Center
        ) {
            Text(glyph, style = WearType.control, color = colors.onBg)
        }
    }
}

/** Logged-set ticks: filled accent = done, hollow = ahead. The section's mark, honest at zero. */
@Composable
fun SetTicks(done: Int, total: Int, modifier: Modifier = Modifier) {
    val colors = LocalWearColors.current
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        repeat(total.coerceIn(0, 8)) { i ->
            val dot = Modifier
                .size(7.dp)
                .clip(CircleShape)
            Box(
                if (i < done) dot.background(colors.accent)
                else dot.border(1.dp, colors.muted.copy(alpha = 0.55f), CircleShape)
            )
        }
    }
}

/** The countdown ring around a timer figure — accent stroke on a faint track. */
@Composable
fun TimerRing(progress: Float, modifier: Modifier = Modifier) {
    val colors = LocalWearColors.current
    CircularProgressIndicator(
        progress = progress.coerceIn(0f, 1f),
        modifier = modifier,
        indicatorColor = colors.accent,
        trackColor = colors.outline.copy(alpha = 0.35f),
        strokeWidth = 4.dp
    )
}
