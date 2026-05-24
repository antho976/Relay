package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.state.VolumePoint
import java.time.Instant
import java.time.ZoneId
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.sin

// ─── Volume per exercise over time (#72) ─────────────────────────────────────

@Composable
fun ExerciseVolumeChart(
    exerciseId: String,
    points: List<VolumePoint>,
    modifier: Modifier = Modifier
) {
    if (points.size < 2) return
    val exerciseName = Program.exercise(exerciseId)?.name ?: exerciseId
    val primaryColor = MaterialTheme.colorScheme.primary
    val surfaceVar = MaterialTheme.colorScheme.surfaceVariant
    val maxVol = points.maxOf { it.totalVolumeLb }.coerceAtLeast(1.0)
    val zone = ZoneId.systemDefault()

    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(surfaceVar, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text("VOLUME OVER TIME · ${exerciseName.uppercase()} · lb",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontWeight = FontWeight.SemiBold)
        Canvas(modifier = Modifier.fillMaxWidth().height(80.dp)) {
            val step = size.width / (points.size - 1).coerceAtLeast(1)
            val path = Path()
            points.forEachIndexed { i, pt ->
                val x = i * step
                val y = size.height - (pt.totalVolumeLb / maxVol * size.height).toFloat()
                if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
            }
            drawPath(path, color = primaryColor, style = Stroke(width = 3f))
            points.forEachIndexed { i, pt ->
                val x = i * step
                val y = size.height - (pt.totalVolumeLb / maxVol * size.height).toFloat()
                drawCircle(primaryColor, radius = 4f, center = Offset(x, y))
            }
        }
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            val first = Instant.ofEpochMilli(points.first().sessionDate).atZone(zone).toLocalDate()
            val last = Instant.ofEpochMilli(points.last().sessionDate).atZone(zone).toLocalDate()
            Text(first.toString().substring(5), style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text("${maxVol.toInt()} lb peak", style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.primary)
            Text(last.toString().substring(5), style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

// ─── Strength Radar Chart (#124) ─────────────────────────────────────────────

private val RADAR_IDS = listOf("ua1", "ua2", "la1", "ub1", "ub2", "lb1")

@Composable
fun StrengthRadarCard(compoundMaxes: Map<String, Double>, modifier: Modifier = Modifier) {
    if (compoundMaxes.isEmpty()) return
    val values = RADAR_IDS.map { id -> compoundMaxes[id] ?: 0.0 }
    if (values.all { it == 0.0 }) return
    val labels = RADAR_IDS.map { Program.exercise(it)?.name?.take(8) ?: it }
    val maxVal = values.max().coerceAtLeast(1.0)
    val primaryColor = MaterialTheme.colorScheme.primary
    val surfaceVar = MaterialTheme.colorScheme.surfaceVariant
    val outlineColor = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f)

    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(surfaceVar, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text("STRENGTH RADAR · lb",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontWeight = FontWeight.SemiBold)
        Canvas(modifier = Modifier.size(200.dp)) {
            val cx = size.width / 2
            val cy = size.height / 2
            val r = min(cx, cy) * 0.8f
            val n = values.size
            val angleStep = (2 * Math.PI / n).toFloat()

            // Grid circles
            listOf(0.25f, 0.5f, 0.75f, 1f).forEach { fraction ->
                val gridPath = Path()
                for (i in 0 until n) {
                    val angle = (i * angleStep - Math.PI / 2).toFloat()
                    val x = cx + cos(angle) * r * fraction
                    val y = cy + sin(angle) * r * fraction
                    if (i == 0) gridPath.moveTo(x, y) else gridPath.lineTo(x, y)
                }
                gridPath.close()
                drawPath(gridPath, color = outlineColor, style = Stroke(1f))
            }

            // Data polygon
            val dataPath = Path()
            values.forEachIndexed { i, v ->
                val angle = (i * angleStep - Math.PI / 2).toFloat()
                val vr = (v / maxVal * r).toFloat()
                val x = cx + cos(angle) * vr
                val y = cy + sin(angle) * vr
                if (i == 0) dataPath.moveTo(x, y) else dataPath.lineTo(x, y)
            }
            dataPath.close()
            drawPath(dataPath, color = primaryColor.copy(alpha = 0.25f))
            drawPath(dataPath, color = primaryColor, style = Stroke(2f))

            // Dots
            values.forEachIndexed { i, v ->
                val angle = (i * angleStep - Math.PI / 2).toFloat()
                val vr = (v / maxVal * r).toFloat()
                drawCircle(primaryColor, radius = 5f, center = Offset(cx + cos(angle) * vr, cy + sin(angle) * vr))
            }
        }
        // Legend
        labels.forEachIndexed { i, label ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Text(label, style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text("${values[i].toInt()} lb", style = MaterialTheme.typography.labelSmall,
                    color = if (values[i] == values.max()) MaterialTheme.colorScheme.primary
                            else MaterialTheme.colorScheme.onSurfaceVariant,
                    fontWeight = if (values[i] == values.max()) FontWeight.SemiBold else FontWeight.Normal)
            }
        }
    }
}

// ─── PR Clustering Calendar Scatter (#128) ────────────────────────────────────

@Composable
fun PrClusteringCard(prTimestamps: List<Long>, modifier: Modifier = Modifier) {
    if (prTimestamps.size < 3) return
    val zone = ZoneId.systemDefault()
    val DOW_LABELS = listOf("M", "T", "W", "T", "F", "S", "S")
    val dowCounts = IntArray(7)
    prTimestamps.forEach { ms ->
        val dow = Instant.ofEpochMilli(ms).atZone(zone).dayOfWeek.value - 1
        dowCounts[dow]++
    }
    val maxCount = dowCounts.max().coerceAtLeast(1)
    val primary = MaterialTheme.colorScheme.primary

    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Text("PR CLUSTERING · ${prTimestamps.size} total PRs",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontWeight = FontWeight.SemiBold)
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.Bottom
        ) {
            dowCounts.forEachIndexed { i, count ->
                Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text("$count", style = MaterialTheme.typography.labelSmall,
                        color = if (count == maxCount) primary else MaterialTheme.colorScheme.onSurfaceVariant)
                    Box(
                        modifier = Modifier
                            .size(32.dp, (40f * count / maxCount).dp.coerceAtLeast(4.dp))
                            .clip(RoundedCornerShape(topStart = 4.dp, topEnd = 4.dp))
                            .background(primary.copy(alpha = 0.3f + 0.7f * count / maxCount))
                    )
                    Text(DOW_LABELS[i], style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
        Text("PRs cluster on ${DOW_LABELS[dowCounts.indexOfMax()]} days",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

private fun IntArray.indexOfMax(): Int {
    var maxI = 0
    for (i in indices) if (this[i] > this[maxI]) maxI = i
    return maxI
}

// ─── Strength Curve Overlay (#94) — two exercises on same chart ───────────────

@Composable
fun StrengthOverlayCard(
    history1: Pair<String, List<com.forge.app.ui.gym.stats.state.HistoryPoint>>,
    history2: Pair<String, List<com.forge.app.ui.gym.stats.state.HistoryPoint>>,
    modifier: Modifier = Modifier
) {
    if (history1.second.size < 2 && history2.second.size < 2) return
    val primaryColor = MaterialTheme.colorScheme.primary
    val secondaryColor = MaterialTheme.colorScheme.tertiary
    val allWeights = (history1.second + history2.second).map { it.maxWeightLb }
    val maxW = allWeights.max().coerceAtLeast(1.0)
    val allDates = (history1.second + history2.second).map { it.sessionDate }.sorted()
    val minDate = allDates.firstOrNull() ?: return
    val dateRange = (allDates.lastOrNull()?.let { it - minDate } ?: 1L).toFloat().coerceAtLeast(1f)

    Column(
        modifier = modifier.fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text("STRENGTH COMPARISON · lb", style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant, fontWeight = FontWeight.SemiBold)
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            LegendItem(history1.first, primaryColor)
            LegendItem(history2.first, secondaryColor)
        }
        Canvas(modifier = Modifier.fillMaxWidth().height(100.dp)) {
            fun drawCurve(pts: List<com.forge.app.ui.gym.stats.state.HistoryPoint>, color: androidx.compose.ui.graphics.Color) {
                if (pts.size < 2) return
                val path = Path()
                pts.sortedBy { it.sessionDate }.forEachIndexed { i, pt ->
                    val x = (pt.sessionDate - minDate) / dateRange * size.width
                    val y = size.height - (pt.maxWeightLb / maxW * size.height).toFloat()
                    if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
                }
                drawPath(path, color, style = Stroke(3f))
            }
            drawCurve(history1.second, primaryColor)
            drawCurve(history2.second, secondaryColor)
        }
    }
}

private fun LegendItem(label: String, color: androidx.compose.ui.graphics.Color) {}

// ─── Effort over time (#95) — mood trend ─────────────────────────────────────

private val MOOD_VALUES = mapOf("drained" to 1, "off" to 2, "fine" to 3, "good" to 4, "strong" to 5)

@Composable
fun EffortOverTimeCard(
    moodData: List<com.forge.app.data.db.dao.SessionDao.MoodOverTime>,
    modifier: Modifier = Modifier
) {
    if (moodData.size < 3) return
    val recentPoints = moodData.takeLast(20)
    val primaryColor = MaterialTheme.colorScheme.primary
    Column(
        modifier = modifier.fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text("ENERGY TREND · last ${recentPoints.size} sessions",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant, fontWeight = FontWeight.SemiBold)
        Canvas(modifier = Modifier.fillMaxWidth().height(80.dp)) {
            val step = size.width / (recentPoints.size - 1).coerceAtLeast(1)
            val path = Path()
            recentPoints.forEachIndexed { i, pt ->
                val score = MOOD_VALUES[pt.mood.lowercase()] ?: 3
                val x = i * step
                val y = size.height - (score / 5f * size.height)
                if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
            }
            drawPath(path, primaryColor, style = Stroke(3f))
            recentPoints.forEachIndexed { i, pt ->
                val score = MOOD_VALUES[pt.mood.lowercase()] ?: 3
                drawCircle(primaryColor, 4f, center = Offset(i * step, size.height - score / 5f * size.height))
            }
        }
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("Drained", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text("Strong", style = MaterialTheme.typography.labelSmall, color = primaryColor)
        }
    }
}
