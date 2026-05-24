package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.Session
import com.forge.app.program.Program
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter

/**
 * Side-by-side comparison of two sessions (#129).
 * Caller is responsible for fetching the two Session objects.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SessionCompareSheet(
    sessionA: Session,
    sessionB: Session,
    onDismiss: () -> Unit
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val zone = ZoneId.systemDefault()
    val fmt = DateTimeFormatter.ofPattern("MMM d")

    fun Session.dateStr() = Instant.ofEpochMilli(startedAt).atZone(zone).format(fmt)
    fun Session.dayName() = Program.days.firstOrNull { it.key == dayKey }?.defaultName ?: dayKey
    fun Session.durationMin() = if (finishedAt != null) ((finishedAt - startedAt) / 60_000).toInt() else 0

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheetState) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text("SESSION COMPARISON", style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.Black)

            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
                    .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text("", modifier = Modifier.weight(1.3f))
                    Text(sessionA.dateStr(), style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
                    Text(sessionB.dateStr(), style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
                }
                HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.2f))

                CompareRow("Day", sessionA.dayName(), sessionB.dayName())
                CompareRow("Volume", "${(sessionA.totalVolumeLb ?: 0.0).toInt()} lb", "${(sessionB.totalVolumeLb ?: 0.0).toInt()} lb",
                    highlight = true, aIsBetter = (sessionA.totalVolumeLb ?: 0.0) > (sessionB.totalVolumeLb ?: 0.0))
                CompareRow("Sets", "${sessionA.setCount}", "${sessionB.setCount}",
                    highlight = true, aIsBetter = sessionA.setCount > sessionB.setCount)
                CompareRow("PRs", "${sessionA.prCount}", "${sessionB.prCount}",
                    highlight = true, aIsBetter = sessionA.prCount > sessionB.prCount)
                CompareRow("Duration", "${sessionA.durationMin()}m", "${sessionB.durationMin()}m")
                if (sessionA.intensity.isNotEmpty() || sessionB.intensity.isNotEmpty()) {
                    CompareRow("Intensity", sessionA.intensity.ifEmpty { "—" }, sessionB.intensity.ifEmpty { "—" })
                }
                if (sessionA.tags.isNotEmpty() || sessionB.tags.isNotEmpty()) {
                    CompareRow("Tags", sessionA.tags.ifEmpty { "none" }, sessionB.tags.ifEmpty { "none" })
                }
            }

            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun CompareRow(
    label: String,
    aVal: String,
    bVal: String,
    highlight: Boolean = false,
    aIsBetter: Boolean = false
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1.3f))
        Text(aVal,
            style = MaterialTheme.typography.bodySmall,
            fontWeight = if (highlight && aIsBetter) FontWeight.Bold else FontWeight.Normal,
            color = if (highlight && aIsBetter) MaterialTheme.colorScheme.primary
                    else MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f))
        Text(bVal,
            style = MaterialTheme.typography.bodySmall,
            fontWeight = if (highlight && !aIsBetter) FontWeight.Bold else FontWeight.Normal,
            color = if (highlight && !aIsBetter) MaterialTheme.colorScheme.primary
                    else MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f))
    }
}
