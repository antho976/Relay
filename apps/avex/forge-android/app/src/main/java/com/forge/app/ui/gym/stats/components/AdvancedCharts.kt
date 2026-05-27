package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.state.VolumePoint
import java.time.Instant
import java.time.ZoneId

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
