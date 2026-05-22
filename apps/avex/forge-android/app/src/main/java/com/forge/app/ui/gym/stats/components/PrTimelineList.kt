package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.ui.gym.stats.state.PrEntry
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun PrTimelineList(
    entries: List<PrEntry>,
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(12.dp)
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                "PR TIMELINE",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontWeight = FontWeight.SemiBold
            )
            if (entries.isEmpty()) {
                Text(
                    "No PRs yet. Log a heavier set than your previous best.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                return@Column
            }
            entries.forEach { entry -> EntryRow(entry) }
        }
    }
}

@Composable
private fun EntryRow(entry: PrEntry) {
    val accent = MaterialTheme.colorScheme.primary
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            formatDate(entry.date),
            modifier = Modifier.padding(end = 8.dp),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Text(
            entry.exerciseName,
            modifier = Modifier.weight(1f),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface
        )
        Text(
            if (entry.reps > 0) "${entry.weightText} × ${entry.reps}" else entry.weightText,
            style = MaterialTheme.typography.bodyMedium,
            color = accent,
            fontWeight = FontWeight.SemiBold
        )
    }
}

private val dateFormat = SimpleDateFormat("MMM d", Locale.getDefault())

private fun formatDate(epochMs: Long): String = dateFormat.format(Date(epochMs))
