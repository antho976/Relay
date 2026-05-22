package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.ui.gym.stats.state.StrengthCurve

@Composable
fun StrengthCurveCard(
    curve: StrengthCurve?,
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(12.dp)
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Text(
                "STRENGTH CURVE",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontWeight = FontWeight.SemiBold
            )
            if (curve == null || curve.points.size < 2) {
                Text(
                    "Log at least 2 sessions to see a strength curve.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                return@Column
            }
            Text(
                curve.plan.name,
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSurface
            )
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    "Now: ${curve.points.last().toInt()} lb",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.primary
                )
                Text(
                    "Best: ${curve.points.max().toInt()} lb",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            SparklineWithAxis(
                values = curve.points,
                lineColor = MaterialTheme.colorScheme.primary,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(88.dp)
                    .padding(top = 4.dp)
            )
            // Legend
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                Box(
                    Modifier
                        .width(14.dp)
                        .height(2.dp)
                        .background(MaterialTheme.colorScheme.primary, RoundedCornerShape(1.dp))
                )
                Text(
                    "max weight · session · lb",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@Composable
private fun SparklineWithAxis(
    values: List<Double>,
    lineColor: Color,
    modifier: Modifier = Modifier
) {
    if (values.size < 2) return
    val minValue = values.min()
    val maxValue = values.max()
    val midValue = (minValue + maxValue) / 2
    val labelStyle = MaterialTheme.typography.labelSmall
    val labelColor = MaterialTheme.colorScheme.onSurfaceVariant

    Row(modifier = modifier, verticalAlignment = Alignment.CenterVertically) {
        Column(
            modifier = Modifier
                .width(36.dp)
                .fillMaxHeight(),
            verticalArrangement = Arrangement.SpaceBetween,
            horizontalAlignment = Alignment.End
        ) {
            Text("${maxValue.toInt()}", style = labelStyle, color = labelColor)
            Text("${midValue.toInt()}", style = labelStyle, color = labelColor)
            Text("${minValue.toInt()}", style = labelStyle, color = labelColor)
        }
        Spacer(Modifier.width(6.dp))
        Sparkline(
            values = values,
            lineColor = lineColor,
            minValue = minValue,
            maxValue = maxValue,
            modifier = Modifier
                .weight(1f)
                .fillMaxHeight()
        )
    }
}

@Composable
private fun Sparkline(
    values: List<Double>,
    lineColor: Color,
    minValue: Double,
    maxValue: Double,
    modifier: Modifier = Modifier
) {
    if (values.size < 2) return
    val range = (maxValue - minValue).coerceAtLeast(1.0)
    val gridColor = lineColor.copy(alpha = 0.12f)

    Canvas(modifier = modifier) {
        val dashPx = 4.dp.toPx()
        val gridEffect = PathEffect.dashPathEffect(floatArrayOf(dashPx, dashPx))

        // Horizontal grid lines at 25 / 50 / 75 %
        listOf(0.25f, 0.50f, 0.75f).forEach { frac ->
            drawLine(
                color = gridColor,
                start = Offset(0f, size.height * frac),
                end = Offset(size.width, size.height * frac),
                strokeWidth = 1.dp.toPx(),
                pathEffect = gridEffect
            )
        }

        val stepX = size.width / (values.size - 1)
        val strokeWidthPx = 2.dp.toPx()
        val path = Path()
        values.forEachIndexed { i, value ->
            val x = stepX * i
            val y = size.height - ((value - minValue) / range * size.height).toFloat()
            if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
        }
        drawPath(path = path, color = lineColor, style = Stroke(width = strokeWidthPx))

        val firstX = 0f
        val firstY = size.height - ((values.first() - minValue) / range * size.height).toFloat()
        val lastX = stepX * (values.size - 1)
        val lastY = size.height - ((values.last() - minValue) / range * size.height).toFloat()
        drawCircle(color = lineColor.copy(alpha = 0.5f), radius = 3.dp.toPx(), center = Offset(firstX, firstY))
        drawCircle(color = lineColor, radius = 4.dp.toPx(), center = Offset(lastX, lastY))
    }
}
