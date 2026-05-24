package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
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
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter

/**
 * Scrollable visual timeline of PRs for a single exercise (#93).
 * Shows each PR as a node on a vertical timeline with date + weight.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PrTimelineSheet(
    exerciseName: String,
    history: List<HistoryPoint>,
    onDismiss: () -> Unit
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val zone = ZoneId.systemDefault()
    val fmt = DateTimeFormatter.ofPattern("MMM d, yyyy")

    // Filter to only PR moments (strictly increasing max weight)
    val prPoints = mutableListOf<HistoryPoint>()
    var maxSoFar = Double.MIN_VALUE
    history.sortedBy { it.sessionDate }.forEach { pt ->
        if (pt.maxWeightLb > maxSoFar) {
            maxSoFar = pt.maxWeightLb
            prPoints.add(pt)
        }
    }

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheetState) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                "PR TIMELINE",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                fontWeight = FontWeight.Black
            )
            Text(exerciseName, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
            if (prPoints.isEmpty()) {
                Text("No PRs yet.", style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            } else {
                LazyColumn(
                    modifier = Modifier.height(400.dp),
                    verticalArrangement = Arrangement.spacedBy(0.dp)
                ) {
                    items(prPoints.reversed(), key = { it.sessionDate }) { pt ->
                        TimelineNode(
                            weight = "${pt.maxWeightLb.toInt()} lb",
                            date = Instant.ofEpochMilli(pt.sessionDate).atZone(zone).format(fmt),
                            isFirst = pt == prPoints.last()
                        )
                    }
                }
            }
            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun TimelineNode(weight: String, date: String, isFirst: Boolean) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Box(
                Modifier
                    .size(12.dp)
                    .background(
                        if (isFirst) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.primary.copy(alpha = 0.4f),
                        CircleShape
                    )
            )
            if (!isFirst) {
                Box(Modifier.width(2.dp).height(40.dp)
                    .background(MaterialTheme.colorScheme.outline.copy(alpha = 0.25f)))
            }
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(weight, style = MaterialTheme.typography.titleMedium,
                fontWeight = if (isFirst) FontWeight.Black else FontWeight.SemiBold,
                color = if (isFirst) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.onSurface)
            Text(date, style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}
