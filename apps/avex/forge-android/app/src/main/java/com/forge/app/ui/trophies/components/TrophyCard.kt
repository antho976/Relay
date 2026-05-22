package com.forge.app.ui.trophies.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.forge.app.ui.trophies.state.TrophyDisplay
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * One tile in the trophies grid. Two visual states:
 *   - Unlocked: full-colour icon badge, accent border, unlock date at the bottom.
 *   - Locked: muted icon, no border, progress hint at the bottom (e.g. "12 / 25").
 *
 * Sizes itself to the content; pair with [androidx.compose.foundation.lazy.grid.LazyVerticalGrid]
 * + GridCells.Fixed(2) for the catalog layout.
 */
@Composable
fun TrophyCard(
    display: TrophyDisplay,
    modifier: Modifier = Modifier
) {
    val unlocked = display.isUnlocked
    val border = if (unlocked) BorderStroke(1.dp, MaterialTheme.colorScheme.primary.copy(alpha = 0.55f)) else null
    val titleColor = if (unlocked) MaterialTheme.colorScheme.onSurface
                     else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.65f)
    val bodyColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = if (unlocked) 1f else 0.75f)
    Surface(
        modifier = modifier
            .fillMaxWidth()
            .heightIn(min = 170.dp),
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(14.dp),
        border = border,
        tonalElevation = if (unlocked) 2.dp else 0.dp
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 14.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            TrophyIconBadge(
                icon = display.trophy.icon,
                unlocked = unlocked,
                size = 48.dp
            )
            Text(
                text = display.trophy.name,
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.SemiBold,
                color = titleColor,
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis
            )
            Text(
                text = display.trophy.description,
                style = MaterialTheme.typography.bodySmall,
                color = bodyColor,
                textAlign = TextAlign.Center,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.fillMaxWidth()
            )
            FooterLine(display)
        }
    }
}

@Composable
private fun FooterLine(display: TrophyDisplay) {
    val text: String
    val color = if (display.isUnlocked) MaterialTheme.colorScheme.primary
                else MaterialTheme.colorScheme.onSurfaceVariant
    if (display.isUnlocked) {
        text = "Unlocked " + formatDate(display.unlockedAt!!)
    } else {
        text = display.progressHint ?: "Locked"
    }
    Text(
        text = text,
        style = MaterialTheme.typography.labelSmall,
        fontWeight = if (display.isUnlocked) FontWeight.SemiBold else FontWeight.Normal,
        color = color,
        textAlign = TextAlign.Center
    )
}

private val dateFormat = SimpleDateFormat("MMM d", Locale.getDefault())

private fun formatDate(epochMs: Long): String = dateFormat.format(Date(epochMs))
