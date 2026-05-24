package com.forge.app.ui.overview.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Compact streak counter row. Icon tier changes at 7 / 30 / 100 consecutive training days.
 */
@Composable
fun StreakCard(
    streakDays: Int,
    modifier: Modifier = Modifier
) {
    val (icon, labelSuffix, tint) = streakTier(streakDays)
    val bg = tint.copy(alpha = 0.12f)

    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(bg)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(icon, fontSize = 26.sp)
        Column(Modifier.weight(1f)) {
            Text(
                text = "$streakDays day${if (streakDays != 1) "s" else ""} $labelSuffix",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                color = tint
            )
            Text(
                text = "STREAK",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                letterSpacing = 1.sp
            )
        }
    }
}

private data class StreakTier(val icon: String, val label: String, val tint: Color)

@Composable
private fun streakTier(days: Int): StreakTier {
    val primary = MaterialTheme.colorScheme.primary
    val onSurface = MaterialTheme.colorScheme.onSurface
    return when {
        days >= 100 -> StreakTier("🏆", "in a row", Color(0xFFFFD700))
        days >= 30  -> StreakTier("🔥", "in a row", primary)
        days >= 7   -> StreakTier("🔥", "in a row", primary.copy(alpha = 0.85f))
        else        -> StreakTier("🔥", "in a row", onSurface.copy(alpha = 0.55f))
    }
}
