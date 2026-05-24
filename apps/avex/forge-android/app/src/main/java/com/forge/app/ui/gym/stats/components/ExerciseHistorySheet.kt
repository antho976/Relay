package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.ui.gym.stats.state.HistoryPoint
import com.forge.app.ui.gym.stats.state.VolumePoint
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Bottom sheet showing the weight-over-time history for one exercise (#27).
 * X-range labels (first/last date) are shown beneath the sparkline.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ExerciseHistorySheet(
    exerciseName: String,
    history: List<HistoryPoint>,
    volumeHistory: List<VolumePoint> = emptyList(),
    onOpenTimeline: (() -> Unit)? = null,
    onDismiss: () -> Unit
) {
    val sheetState = rememberModalBottomSheetState()

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surface
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp)
                .padding(bottom = 8.dp)
                .navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(
                exerciseName,
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.onSurface
            )

            if (history.size < 2) {
                Text(
                    "Log at least 2 sessions to see a history graph.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.height(8.dp))
                return@Column
            }

            val values = history.map { it.maxWeightLb }
            val best = values.max()
            val latest = values.last()

            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                StatChip(label = "Latest", value = "${latest.toInt()} lb")
                StatChip(label = "Best", value = "${best.toInt()} lb")
                StatChip(label = "Sessions", value = "${history.size}")
            }

            SparklineWithAxis(
                values = values,
                lineColor = MaterialTheme.colorScheme.primary,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(110.dp)
                    .padding(top = 4.dp)
            )

            onOpenTimeline?.let { openTimeline ->
                androidx.compose.material3.TextButton(onClick = openTimeline) {
                    androidx.compose.material3.Text("View PR timeline →")
                }
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Text(
                    formatDate(history.first().sessionDate),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Text(
                    formatDate(history.last().sessionDate),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                Box(
                    Modifier
                        .padding(start = 2.dp)
                        .height(2.dp)
                        .padding(horizontal = 2.dp)
                )
                Text(
                    "max weight per session · lb",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            // Volume over time chart (#72) shown below weight chart if data available
            if (volumeHistory.size >= 2) {
                val volValues = volumeHistory.map { it.totalVolumeLb }
                val maxVol = volValues.max()
                Text("Volume per session · lb", style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                SparklineWithAxis(
                    values = volValues,
                    lineColor = MaterialTheme.colorScheme.tertiary,
                    modifier = Modifier.fillMaxWidth().height(80.dp).padding(top = 4.dp)
                )
                Text("${maxVol.toInt()} lb peak volume", style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.tertiary)
            }

            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun StatChip(label: String, value: String) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(
            value,
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.onSurface
        )
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

private val dateFormat = SimpleDateFormat("MMM d, yyyy", Locale.getDefault())
private fun formatDate(epochMs: Long): String = dateFormat.format(Date(epochMs))
