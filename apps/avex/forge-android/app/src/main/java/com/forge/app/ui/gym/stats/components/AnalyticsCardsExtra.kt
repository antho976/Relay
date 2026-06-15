package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import kotlin.math.roundToInt
import com.forge.app.ui.gym.stats.state.DayTypeVolumeStats
import com.forge.app.ui.theme.LocalForgeSettings

// VolumeDeloadCard (#126) was retired in the Stats revamp — its weekly-bucketed
// replacement is TonnageTrendCard in StatsVolumeExtra.kt.

@Composable
fun DayTypeBestVsAvgCard(data: List<DayTypeVolumeStats>, modifier: Modifier = Modifier) {
    if (data.isEmpty()) return
    val useKg = LocalForgeSettings.current.useKg
    StatCard(title = "BEST VS AVERAGE · ${unitLabel(useKg)}", modifier = modifier) {
        data.forEach { stats ->
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    stats.dayName,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.weight(1f)
                )
                Column(horizontalAlignment = Alignment.End) {
                    Text(
                        "Best: ${toDisplayWeight(stats.maxVolumeLb, useKg).roundToInt()} ${unitLabel(useKg)}",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.primary,
                        fontWeight = FontWeight.SemiBold
                    )
                    Text(
                        "Avg: ${toDisplayWeight(stats.avgVolumeLb, useKg).roundToInt()} ${unitLabel(useKg)} · ${stats.sessionCount} sessions",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }
    }
}
