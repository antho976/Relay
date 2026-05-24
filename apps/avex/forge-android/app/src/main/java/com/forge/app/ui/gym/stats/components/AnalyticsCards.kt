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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
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
import com.forge.app.ui.gym.stats.state.ExerciseFrequency
import com.forge.app.ui.gym.stats.state.MuscleVolume
import com.forge.app.ui.gym.stats.state.TimeToPrEntry
import com.forge.app.ui.gym.stats.state.VolumeDeloadPoint
import java.time.Instant
import java.time.ZoneId

// ─── Exercise Frequency (#73) ─────────────────────────────────────────────────

@Composable
fun ExerciseFrequencyCard(data: List<ExerciseFrequency>, modifier: Modifier = Modifier) {
    if (data.isEmpty()) return
    StatCard(title = "EXERCISE FREQUENCY · 8 WEEKS", modifier = modifier) {
        data.take(8).forEach { item ->
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 3.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    item.exerciseName,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.weight(1f)
                )
                Spacer(Modifier.width(8.dp))
                FreqBar(item.sessionCount, item.outOf)
                Spacer(Modifier.width(6.dp))
                Text(
                    "${item.sessionCount}×",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.primary,
                    fontWeight = FontWeight.SemiBold
                )
            }
        }
    }
}

@Composable
private fun FreqBar(count: Int, max: Int) {
    val fraction = count.toFloat() / max.coerceAtLeast(1)
    Box(
        modifier = Modifier
            .width(64.dp)
            .height(6.dp)
            .clip(RoundedCornerShape(3.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant)
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth(fraction)
                .height(6.dp)
                .clip(RoundedCornerShape(3.dp))
                .background(MaterialTheme.colorScheme.primary)
        )
    }
}

// ─── Time to Next PR (#74) ────────────────────────────────────────────────────

@Composable
fun TimeToPrCard(data: List<TimeToPrEntry>, modifier: Modifier = Modifier) {
    if (data.isEmpty()) return
    StatCard(title = "TIME BETWEEN PRs", modifier = modifier) {
        data.take(6).forEachIndexed { i, item ->
            if (i > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.15f))
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    item.exerciseName,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.weight(1f)
                )
                Column(horizontalAlignment = Alignment.End) {
                    Text(
                        "~${item.avgDaysBetween}d avg",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.primary,
                        fontWeight = FontWeight.SemiBold
                    )
                    Text(
                        "${item.prCount} PRs total",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }
    }
}

// ─── Day-of-Week PR Distribution (#85) ───────────────────────────────────────

private val DOW_LABELS = listOf("Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun")

@Composable
fun PrDayOfWeekCard(counts: List<Int>, modifier: Modifier = Modifier) {
    if (counts.all { it == 0 }) return
    val max = counts.max().coerceAtLeast(1)
    StatCard(title = "PRs BY DAY OF WEEK", modifier = modifier) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.Bottom
        ) {
            counts.forEachIndexed { i, count ->
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    Text(
                        "$count",
                        style = MaterialTheme.typography.labelSmall,
                        color = if (count == max) MaterialTheme.colorScheme.primary
                               else MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Box(
                        modifier = Modifier
                            .width(24.dp)
                            .height((48 * count.toFloat() / max).dp.coerceAtLeast(4.dp))
                            .clip(RoundedCornerShape(topStart = 4.dp, topEnd = 4.dp))
                            .background(
                                if (count == max) MaterialTheme.colorScheme.primary
                                else MaterialTheme.colorScheme.primary.copy(alpha = 0.35f)
                            )
                    )
                    Text(
                        DOW_LABELS[i],
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }
    }
}

// ─── Volume Distribution Donut (#125) ─────────────────────────────────────────

private val MUSCLE_COLORS = listOf(
    Color(0xFF2196F3), Color(0xFF4CAF50), Color(0xFFFF9800),
    Color(0xFFF44336), Color(0xFF9C27B0), Color(0xFF00BCD4),
    Color(0xFFFFEB3B), Color(0xFF795548), Color(0xFF607D8B),
    Color(0xFFE91E63), Color(0xFF009688)
)

@Composable
fun VolumeDonutCard(rows: List<MuscleVolume>, modifier: Modifier = Modifier) {
    if (rows.isEmpty()) return
    val total = rows.sumOf { it.volumeLb }.coerceAtLeast(1.0)
    StatCard(title = "VOLUME BREAKDOWN · lb", modifier = modifier) {
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            rows.forEachIndexed { i, mv ->
                val pct = (mv.volumeLb / total * 100).toInt()
                val color = MUSCLE_COLORS[i % MUSCLE_COLORS.size]
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Box(Modifier.size(10.dp).background(color, CircleShape))
                    Text(
                        mv.muscle.displayName,
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.weight(1f),
                        color = MaterialTheme.colorScheme.onSurface
                    )
                    Box(
                        modifier = Modifier
                            .width((pct * 0.8f).dp.coerceAtLeast(4.dp))
                            .height(6.dp)
                            .clip(RoundedCornerShape(3.dp))
                            .background(color)
                    )
                    Text(
                        "$pct%",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }
    }
}

// ─── Volume Trend with Deload Markers (#126) ──────────────────────────────────

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

// ─── Best vs Average per Day Type (#132) ──────────────────────────────────────

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

// ─── Shared card shell ────────────────────────────────────────────────────────

@Composable
private fun StatCard(
    title: String,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Text(
            title,
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontWeight = FontWeight.SemiBold
        )
        content()
    }
}
