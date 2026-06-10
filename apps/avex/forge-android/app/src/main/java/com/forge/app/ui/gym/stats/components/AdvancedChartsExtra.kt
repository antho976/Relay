package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.stats.state.PatternAxis
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.sin

/**
 * "Your shape" — movement-pattern radar. Each axis is the pattern's recent best e1RM as a
 * fraction of its all-time best, so the polygon reads as current shape vs your own peak
 * (replaces the old hardcoded-exercise-id radar, which broke on regenerated programs).
 * Editorial: self-padded hairline section, no boxed card. Invoke via a plain item.
 */
@Composable
fun PatternRadarCard(axes: List<PatternAxis>, modifier: Modifier = Modifier) {
    if (axes.size < 3) return
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline
    val progress = rememberDrawProgress(axes.size)

    Column(modifier = modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Text("Your shape", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
        Spacer(Modifier.height(4.dp))
        Text(
            "Each pattern's recent best vs your all-time peak.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(12.dp))
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            Canvas(modifier = Modifier.size(190.dp)) {
                val cx = size.width / 2
                val cy = size.height / 2
                val r = min(cx, cy) * 0.85f
                val n = axes.size
                val angleStep = (2 * Math.PI / n).toFloat()
                listOf(0.25f, 0.5f, 0.75f, 1f).forEach { fraction ->
                    val gridPath = Path()
                    for (i in 0 until n) {
                        val angle = (i * angleStep - Math.PI / 2).toFloat()
                        val x = cx + cos(angle) * r * fraction
                        val y = cy + sin(angle) * r * fraction
                        if (i == 0) gridPath.moveTo(x, y) else gridPath.lineTo(x, y)
                    }
                    gridPath.close()
                    drawPath(gridPath, color = outline.copy(alpha = 0.25f), style = Stroke(1.dp.toPx()))
                }
                // Draw-in: the polygon grows out from the center once on appear.
                val dataPath = Path()
                axes.forEachIndexed { i, axis ->
                    val angle = (i * angleStep - Math.PI / 2).toFloat()
                    val vr = (axis.fraction * r * progress).toFloat()
                    val x = cx + cos(angle) * vr
                    val y = cy + sin(angle) * vr
                    if (i == 0) dataPath.moveTo(x, y) else dataPath.lineTo(x, y)
                }
                dataPath.close()
                drawPath(dataPath, color = accent.copy(alpha = 0.18f))
                drawPath(dataPath, color = accent, style = Stroke(2.dp.toPx()))
                axes.forEachIndexed { i, axis ->
                    val angle = (i * angleStep - Math.PI / 2).toFloat()
                    val vr = (axis.fraction * r * progress).toFloat()
                    drawCircle(accent, radius = 2.5.dp.toPx(), center = Offset(cx + cos(angle) * vr, cy + sin(angle) * vr))
                }
            }
        }
        Spacer(Modifier.height(12.dp))
        axes.forEach { axis ->
            val pct = (axis.fraction * 100).toInt()
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 3.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(axis.label, style = MaterialTheme.typography.labelSmall, color = muted, letterSpacing = 1.sp)
                Text(
                    "${axis.currentE1rm.toInt()} / ${axis.peakE1rm.toInt()} lb · $pct%",
                    style = MaterialTheme.typography.labelSmall,
                    color = if (pct >= 100) accent else onBg.copy(alpha = 0.75f),
                    fontWeight = if (pct >= 100) FontWeight.SemiBold else FontWeight.Normal
                )
            }
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

// PrClusteringCard (#128) was retired in the Stats revamp — it duplicated
// PrDayOfWeekCard's PRs-by-weekday view almost exactly.
