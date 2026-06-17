package com.forge.app.ui.gym.stats.components

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathMeasure
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings

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
    val useKg = LocalForgeSettings.current.useKg

    Column(
        modifier = modifier.fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text("STRENGTH COMPARISON · ${unitLabel(useKg)}", style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant, fontWeight = FontWeight.SemiBold)
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            LegendItem(history1.first, primaryColor)
            LegendItem(history2.first, secondaryColor)
        }
        // Curves draw themselves on first appearance (path-trim 0→1) instead of snapping in.
        val drawProgress = remember { Animatable(0f) }
        LaunchedEffect(Unit) { drawProgress.animateTo(1f, animationSpec = tween(ForgeMotion.scaledDuration(700), easing = ForgeMotion.Decelerate)) }
        Canvas(modifier = Modifier.fillMaxWidth().height(100.dp)
            .semantics { contentDescription = "Line chart comparing estimated 1RM of ${history1.first} and ${history2.first} over time." }) {
            val p = drawProgress.value
            fun drawCurve(pts: List<com.forge.app.ui.gym.stats.state.HistoryPoint>, color: androidx.compose.ui.graphics.Color) {
                if (pts.size < 2) return
                val path = Path()
                pts.sortedBy { it.sessionDate }.forEachIndexed { i, pt ->
                    val x = (pt.sessionDate - minDate) / dateRange * size.width
                    val y = size.height - (pt.maxWeightLb / maxW * size.height).toFloat()
                    if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
                }
                val measure = PathMeasure().apply { setPath(path, false) }
                val drawn = Path()
                measure.getSegment(0f, measure.length * p, drawn, true)
                drawPath(drawn, color, style = Stroke(2.dp.toPx()))
            }
            drawCurve(history1.second, primaryColor)
            drawCurve(history2.second, secondaryColor)
        }
    }
}

@Composable
private fun LegendItem(label: String, color: androidx.compose.ui.graphics.Color) {
    // Was an empty no-op, so the two comparison curves were unlabeled. Colour the label to match
    // its curve.
    Text("● $label", style = MaterialTheme.typography.labelSmall, color = color)
}

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
        val drawProgress = remember { Animatable(0f) }
        LaunchedEffect(Unit) { drawProgress.animateTo(1f, animationSpec = tween(ForgeMotion.scaledDuration(700), easing = ForgeMotion.Decelerate)) }
        Canvas(modifier = Modifier.fillMaxWidth().height(80.dp)
            .semantics { contentDescription = "Energy trend over the last ${recentPoints.size} sessions, from drained to strong." }) {
            val p = drawProgress.value
            val step = size.width / (recentPoints.size - 1).coerceAtLeast(1)
            val path = Path()
            recentPoints.forEachIndexed { i, pt ->
                val score = MOOD_VALUES[pt.mood.lowercase()] ?: 3
                val x = i * step
                val y = size.height - (score / 5f * size.height)
                if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
            }
            val measure = PathMeasure().apply { setPath(path, false) }
            val drawn = Path()
            measure.getSegment(0f, measure.length * p, drawn, true)
            drawPath(drawn, primaryColor, style = Stroke(2.dp.toPx()))
            // Reveal each point's dot as the line reaches it.
            recentPoints.forEachIndexed { i, pt ->
                val frac = if (recentPoints.size <= 1) 1f else i.toFloat() / (recentPoints.size - 1)
                if (frac <= p) {
                    val score = MOOD_VALUES[pt.mood.lowercase()] ?: 3
                    drawCircle(primaryColor, 3.dp.toPx(), center = Offset(i * step, size.height - score / 5f * size.height))
                }
            }
        }
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("Drained", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text("Strong", style = MaterialTheme.typography.labelSmall, color = primaryColor)
        }
    }
}
