package com.forge.app.ui.cardio.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.DeleteOutline
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * One row in the recent-cardio list. Decodes the raw [CardioEntry] strings to enums
 * for icon + display name. Trailing trash button opens the parent's delete confirm.
 */
@Composable
fun CardioEntryRow(
    entry: CardioEntry,
    onRequestDelete: () -> Unit,
    modifier: Modifier = Modifier
) {
    val type = CardioType.fromCode(entry.type)
    Surface(
        modifier = modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(12.dp)
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 14.dp, top = 4.dp, bottom = 4.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            TypeBadge(type)
            Column(modifier = Modifier.weight(1f).padding(vertical = 8.dp)) {
                Text(
                    type.displayName,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                    fontWeight = FontWeight.SemiBold
                )
                Text(
                    subtitleFor(entry, type),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            Text(
                formatDate(entry.date),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            IconButton(onClick = onRequestDelete) {
                Icon(
                    Icons.Outlined.DeleteOutline,
                    contentDescription = "Delete entry",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@Composable
private fun TypeBadge(type: CardioType) {
    val accent = MaterialTheme.colorScheme.primary
    Box(
        modifier = Modifier
            .size(36.dp)
            .clip(CircleShape)
            .background(accent.copy(alpha = 0.18f)),
        contentAlignment = Alignment.Center
    ) {
        Icon(
            imageVector = type.icon,
            contentDescription = null,
            tint = accent,
            modifier = Modifier.size(20.dp)
        )
    }
}

private fun subtitleFor(entry: CardioEntry, type: CardioType): String {
    if (type.isRest) {
        val reason = CardioRestReason.fromCode(entry.restReason)?.displayName
        return reason ?: "Rest day"
    }
    val pieces = mutableListOf<String>()
    if (entry.durationMin > 0) pieces.add("${entry.durationMin} min")
    entry.distanceKm?.let { pieces.add("${formatDistance(it)} km") }
    CardioEffort.fromCode(entry.effort)?.let { pieces.add(it.displayName) }
    return if (pieces.isEmpty()) "—" else pieces.joinToString(" · ")
}

private fun formatDistance(km: Double): String =
    if (km % 1.0 == 0.0) km.toInt().toString() else String.format(Locale.getDefault(), "%.2f", km)

private val dateFormat = SimpleDateFormat("MMM d", Locale.getDefault())
private fun formatDate(epochMs: Long): String = dateFormat.format(Date(epochMs))
