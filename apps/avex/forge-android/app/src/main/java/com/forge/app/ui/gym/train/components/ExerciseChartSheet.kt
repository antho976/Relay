package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.train.state.ExerciseSessionPoint
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

/**
 * Detail chart sheet opened by tapping the last-session strip. Shows a big volume-over-time
 * chart (actual sessions solid, a short projection dashed) plus bullet cards for the most
 * recent sessions.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ExerciseChartSheet(
    exerciseName: String,
    history: List<ExerciseSessionPoint>,
    onDismiss: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val accent = MaterialTheme.colorScheme.primary
    val bg = MaterialTheme.colorScheme.background

    // Chronological (oldest → newest) for the chart.
    val chrono = history.asReversed()
    val volumes = chrono.map { it.volumeLb }
    val zone = ZoneId.systemDefault()
    val dateFmt = DateTimeFormatter.ofPattern("MMM d", Locale.getDefault())

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = bg
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(exerciseName, style = MaterialTheme.typography.headlineSmall, color = onBg)
            Text(
                "VOLUME ACROSS SESSIONS · lb",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 9.sp,
                letterSpacing = 1.sp
            )

            if (volumes.size >= 2) {
                VolumeProjectionChart(
                    volumes = volumes,
                    lineColor = onBg,
                    projectionColor = accent,
                    gridColor = outline.copy(alpha = 0.25f),
                    modifier = Modifier.fillMaxWidth().height(150.dp)
                )
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text("solid = logged", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                    Text("┈ projected", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 9.sp)
                }
            } else {
                Text(
                    "Log a couple of sessions to see the trend.",
                    style = MaterialTheme.typography.bodySmall,
                    color = muted,
                    fontStyle = FontStyle.Italic
                )
            }

            HorizontalDivider(color = outline.copy(alpha = 0.2f))
            Text(
                "RECENT SESSIONS",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 9.sp,
                letterSpacing = 1.sp
            )

            history.take(5).forEach { pt ->
                val dateStr = Instant.ofEpochMilli(pt.sessionStartedAt).atZone(zone).toLocalDate().format(dateFmt)
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .border(0.5.dp, outline.copy(alpha = 0.3f), RoundedCornerShape(8.dp))
                        .padding(horizontal = 14.dp, vertical = 10.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Column {
                        Text(dateStr, style = MaterialTheme.typography.bodyMedium, color = onBg, fontWeight = FontWeight.SemiBold)
                        val meta = buildString {
                            pt.durationMin?.let { append("$it min") }
                            pt.topWeightLb?.let {
                                if (isNotEmpty()) append(" · ")
                                append("top ${it.toInt()} lb")
                            }
                        }
                        if (meta.isNotBlank()) {
                            Text(meta, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
                        }
                    }
                    Text("${pt.volumeLb.toInt()} lb", style = MaterialTheme.typography.bodyMedium, color = accent, fontWeight = FontWeight.SemiBold)
                }
            }

            Spacer(Modifier.height(32.dp))
        }
    }
}

@Composable
private fun VolumeProjectionChart(
    volumes: List<Double>,
    lineColor: androidx.compose.ui.graphics.Color,
    projectionColor: androidx.compose.ui.graphics.Color,
    gridColor: androidx.compose.ui.graphics.Color,
    modifier: Modifier = Modifier
) {
    // Linear projection: continue the last segment's slope for two more points.
    val slope = volumes.last() - volumes[volumes.size - 2]
    val projected = listOf(
        (volumes.last() + slope).coerceAtLeast(0.0),
        (volumes.last() + 2 * slope).coerceAtLeast(0.0)
    )
    val combined = volumes + projected
    val minV = combined.min()
    val maxV = combined.max()
    val range = (maxV - minV).coerceAtLeast(1.0)
    val totalPoints = combined.size

    Canvas(modifier = modifier) {
        val stepX = size.width / (totalPoints - 1).coerceAtLeast(1)
        fun yFor(v: Double) = size.height - ((v - minV) / range * size.height).toFloat()

        listOf(0.25f, 0.5f, 0.75f).forEach { frac ->
            drawLine(
                color = gridColor,
                start = Offset(0f, size.height * frac),
                end = Offset(size.width, size.height * frac),
                strokeWidth = 1.dp.toPx(),
                pathEffect = PathEffect.dashPathEffect(floatArrayOf(4.dp.toPx(), 4.dp.toPx()))
            )
        }

        // Solid actual line
        val actualPath = Path()
        volumes.forEachIndexed { i, v ->
            val x = stepX * i
            val y = yFor(v)
            if (i == 0) actualPath.moveTo(x, y) else actualPath.lineTo(x, y)
        }
        drawPath(actualPath, color = lineColor, style = Stroke(width = 3.dp.toPx()))
        volumes.forEachIndexed { i, v ->
            drawCircle(lineColor, radius = 3.dp.toPx(), center = Offset(stepX * i, yFor(v)))
        }

        // Dashed projected continuation, starting from the last actual point
        val projPath = Path()
        val startIdx = volumes.size - 1
        projPath.moveTo(stepX * startIdx, yFor(volumes.last()))
        projected.forEachIndexed { i, v ->
            projPath.lineTo(stepX * (startIdx + 1 + i), yFor(v))
        }
        drawPath(
            projPath,
            color = projectionColor,
            style = Stroke(width = 2.5.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(8f, 8f)))
        )
    }
}
