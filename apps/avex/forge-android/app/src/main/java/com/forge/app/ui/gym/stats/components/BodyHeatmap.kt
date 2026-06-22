package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.program.MuscleGroup

/** A muscle region as one blob in normalized [0,1] figure coordinates (x,y = top-left). */
private data class NRect(val x: Float, val y: Float, val w: Float, val h: Float)

/** Front-view muscle blobs. A muscle can have several blobs (e.g. left + right arm). */
private val FRONT_REGIONS: Map<MuscleGroup, List<NRect>> = mapOf(
    MuscleGroup.SHOULDERS to listOf(NRect(0.16f, 0.17f, 0.18f, 0.10f), NRect(0.66f, 0.17f, 0.18f, 0.10f)),
    MuscleGroup.CHEST to listOf(NRect(0.30f, 0.22f, 0.40f, 0.12f)),
    MuscleGroup.BICEPS to listOf(NRect(0.10f, 0.30f, 0.14f, 0.16f), NRect(0.76f, 0.30f, 0.14f, 0.16f)),
    MuscleGroup.CORE to listOf(NRect(0.34f, 0.36f, 0.32f, 0.18f)),
    MuscleGroup.QUADS to listOf(NRect(0.28f, 0.58f, 0.18f, 0.24f), NRect(0.54f, 0.58f, 0.18f, 0.24f)),
)

/** Back-view muscle blobs. */
private val BACK_REGIONS: Map<MuscleGroup, List<NRect>> = mapOf(
    MuscleGroup.REAR_DELTS to listOf(NRect(0.16f, 0.17f, 0.18f, 0.10f), NRect(0.66f, 0.17f, 0.18f, 0.10f)),
    MuscleGroup.BACK to listOf(NRect(0.28f, 0.22f, 0.44f, 0.22f)),
    MuscleGroup.TRICEPS to listOf(NRect(0.10f, 0.30f, 0.14f, 0.16f), NRect(0.76f, 0.30f, 0.14f, 0.16f)),
    MuscleGroup.GLUTES to listOf(NRect(0.30f, 0.48f, 0.40f, 0.13f)),
    MuscleGroup.HAMSTRINGS to listOf(NRect(0.28f, 0.62f, 0.18f, 0.20f), NRect(0.54f, 0.62f, 0.18f, 0.20f)),
    MuscleGroup.CALVES to listOf(NRect(0.30f, 0.84f, 0.15f, 0.13f), NRect(0.55f, 0.84f, 0.15f, 0.13f)),
)

/**
 * Two stylized figures (front + back) with each muscle region tinted by its weekly set count —
 * faint = neglected, bold accent = most-trained. The "where am I neglecting?" read the spec calls the
 * headline visual. Deliberately schematic (rounded blobs, not anatomy): legible at a glance and cheap
 * to draw. Intensity is relative to the busiest muscle this week.
 */
@Composable
internal fun BodyHeatmap(
    setsByMuscle: Map<MuscleGroup, Int>,
    accent: Color,
    faint: Color,
    silhouette: Color,
    labelColor: Color,
    modifier: Modifier = Modifier
) {
    val maxSets = (setsByMuscle.values.maxOrNull() ?: 0).coerceAtLeast(1)
    Column(modifier) {
        Row(Modifier.fillMaxWidth()) {
            FigureColumn("FRONT", FRONT_REGIONS, setsByMuscle, maxSets, accent, faint, silhouette, labelColor, Modifier.weight(1f))
            FigureColumn("BACK", BACK_REGIONS, setsByMuscle, maxSets, accent, faint, silhouette, labelColor, Modifier.weight(1f))
        }
        Spacer(Modifier.height(10.dp))
        HeatLegend(accent = accent, faint = faint, labelColor = labelColor)
    }
}

@Composable
private fun FigureColumn(
    title: String,
    regions: Map<MuscleGroup, List<NRect>>,
    setsByMuscle: Map<MuscleGroup, Int>,
    maxSets: Int,
    accent: Color,
    faint: Color,
    silhouette: Color,
    labelColor: Color,
    modifier: Modifier = Modifier
) {
    Column(modifier, horizontalAlignment = Alignment.CenterHorizontally) {
        Text(title, style = MaterialTheme.typography.labelSmall, color = labelColor)
        Spacer(Modifier.height(6.dp))
        Canvas(Modifier.fillMaxWidth().height(220.dp).padding(horizontal = 8.dp)) {
            drawSilhouette(silhouette)
            regions.forEach { (muscle, blobs) ->
                val sets = setsByMuscle[muscle] ?: 0
                val color = lerp(faint, accent, sets.toFloat() / maxSets)
                blobs.forEach { n ->
                    drawRoundRect(
                        color = color,
                        topLeft = Offset(n.x * size.width, n.y * size.height),
                        size = Size(n.w * size.width, n.h * size.height),
                        cornerRadius = CornerRadius(6.dp.toPx(), 6.dp.toPx())
                    )
                }
            }
        }
    }
}

/** A faint humanoid backdrop so the colored blobs read as "on a body", not floating. */
private fun androidx.compose.ui.graphics.drawscope.DrawScope.drawSilhouette(color: Color) {
    val w = size.width
    val h = size.height
    // head
    drawCircle(color = color, radius = 0.07f * h, center = Offset(0.5f * w, 0.08f * h))
    // torso
    drawRoundRect(color = color, topLeft = Offset(0.28f * w, 0.15f * h), size = Size(0.44f * w, 0.42f * h), cornerRadius = CornerRadius(14f, 14f))
    // arms
    drawRoundRect(color = color, topLeft = Offset(0.09f * w, 0.16f * h), size = Size(0.15f * w, 0.34f * h), cornerRadius = CornerRadius(14f, 14f))
    drawRoundRect(color = color, topLeft = Offset(0.76f * w, 0.16f * h), size = Size(0.15f * w, 0.34f * h), cornerRadius = CornerRadius(14f, 14f))
    // legs
    drawRoundRect(color = color, topLeft = Offset(0.30f * w, 0.56f * h), size = Size(0.18f * w, 0.42f * h), cornerRadius = CornerRadius(14f, 14f))
    drawRoundRect(color = color, topLeft = Offset(0.52f * w, 0.56f * h), size = Size(0.18f * w, 0.42f * h), cornerRadius = CornerRadius(14f, 14f))
}

@Composable
private fun HeatLegend(accent: Color, faint: Color, labelColor: Color) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text("less", style = MaterialTheme.typography.labelSmall, color = labelColor)
        Canvas(
            Modifier
                .padding(horizontal = 8.dp)
                .weight(1f)
                .height(8.dp)
                .clip(androidx.compose.foundation.shape.RoundedCornerShape(4.dp))
        ) {
            val steps = 24
            val stepW = size.width / steps
            for (i in 0 until steps) {
                drawRect(
                    color = lerp(faint, accent, i / (steps - 1f)),
                    topLeft = Offset(i * stepW, 0f),
                    size = Size(stepW + 1f, size.height)
                )
            }
        }
        Text("more sets", style = MaterialTheme.typography.labelSmall, color = labelColor, textAlign = TextAlign.End)
    }
}
