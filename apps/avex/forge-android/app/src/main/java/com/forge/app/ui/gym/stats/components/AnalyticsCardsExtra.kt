package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.weight
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
import com.forge.app.ui.gym.stats.state.DayTypeVolumeStats
import com.forge.app.ui.gym.stats.state.VolumeDeloadPoint
import java.time.Instant
import java.time.ZoneId

@Composable
fun VolumeDeloadCard(points: List<VolumeDeloadPoint>, modifier: Modifier = Modifier) {
    if (points.isEmpty()) return
    val max = points.maxOf { it.totalVolumeLb }.coerceAtLeast(1.0)
    val zone = ZoneId.systemDefault()
    StatCard(title = "VOLUME TREND (last 30 sessions)", modifier = modifier) {
        Row(
            modifier = Modifier.fillMaxWidth().height(80.dp),
            horizontalArrangement = Arrangement.spacedBy(2.dp),
            verticalAlignment = Alignment.Bottom
        ) {
            points.forEach { pt ->
                val fraction = (pt.totalVolumeLb / max).toFloat()
                val color = if (pt.isDeload) Color(0xFFFF9800) else MaterialTheme.colorScheme.primary
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth()
                        .height((fraction * 72).dp.coerceAtLeast(2.dp))
                        .clip(RoundedCornerShape(topStart = 2.dp, topEnd = 2.dp))
                        .background(color.copy(alpha = if (pt.isDeload) 0.85f else 0.7f))
                )
            }
        }
        Row(
            modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            val firstDate = Instant.ofEpochMilli(points.first().sessionDate).atZone(zone).toLocalDate()
            val lastDate = Instant.ofEpochMilli(points.last().sessionDate).atZone(zone).toLocalDate()
            Text(firstDate.toString().substring(5), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text("🟠 = deload", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(lastDate.toString().substring(5), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
fun DayTypeBestVsAvgCard(data: List<DayTypeVolumeStats>, modifier: Modifier = Modifier) {
    if (data.isEmpty()) return
    StatCard(title = "BEST VS AVERAGE · lb", modifier = modifier) {
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
                        "Best: ${stats.maxVolumeLb.toInt()} lb",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.primary,
                        fontWeight = FontWeight.SemiBold
                    )
                    Text(
                        "Avg: ${stats.avgVolumeLb.toInt()} lb · ${stats.sessionCount} sessions",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }
    }
}
