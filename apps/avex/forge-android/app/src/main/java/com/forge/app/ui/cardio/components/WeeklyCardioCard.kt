package com.forge.app.ui.cardio.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * This-week summary at the top of the cardio screen. Excludes rest days from the
 * minutes count (the underlying query already filters them out). When [weekDailyMinutes]
 * is provided (7 values Mon–Sun) a mini bar chart is shown beneath the stats.
 */
@Composable
fun WeeklyCardioCard(
    minutes: Int,
    entries: Int,
    weekDailyMinutes: List<Int> = emptyList(),
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(14.dp)
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 14.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                "THIS WEEK",
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(24.dp),
                verticalAlignment = Alignment.Bottom
            ) {
                BigStat(value = "$minutes", unit = "min", modifier = Modifier.weight(1f))
                BigStat(
                    value = "$entries",
                    unit = if (entries == 1) "session" else "sessions",
                    modifier = Modifier.weight(1f)
                )
            }
            if (weekDailyMinutes.size == 7) {
                WeeklyMiniChart(
                    dailyMinutes = weekDailyMinutes,
                    accentColor = MaterialTheme.colorScheme.primary,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(48.dp)
                )
            }
        }
    }
}

@Composable
private fun BigStat(value: String, unit: String, modifier: Modifier = Modifier) {
    Column(modifier = modifier) {
        Text(
            value,
            fontSize = 32.sp,
            fontWeight = FontWeight.Black,
            color = MaterialTheme.colorScheme.onSurface
        )
        Text(
            unit,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

@Composable
private fun WeeklyMiniChart(
    dailyMinutes: List<Int>,
    accentColor: Color,
    modifier: Modifier = Modifier
) {
    val dayLabels = listOf("M", "T", "W", "T", "F", "S", "S")
    val max = (dailyMinutes.maxOrNull() ?: 0).coerceAtLeast(1)
    val barColor = accentColor.copy(alpha = 0.75f)
    val emptyColor = accentColor.copy(alpha = 0.12f)

    Column(modifier = modifier) {
        Canvas(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
        ) {
            val barCount = dailyMinutes.size
            val barWidth = size.width / barCount
            val barPad = barWidth * 0.18f

            dailyMinutes.forEachIndexed { i, mins ->
                val frac = if (mins > 0) mins.toFloat() / max else 0.05f
                val barH = size.height * frac
                val left = i * barWidth + barPad
                val right = (i + 1) * barWidth - barPad
                drawRoundRect(
                    color = if (mins > 0) barColor else emptyColor,
                    topLeft = Offset(left, size.height - barH),
                    size = Size(right - left, barH),
                    cornerRadius = CornerRadius(3.dp.toPx())
                )
            }
        }
        Row(modifier = Modifier.fillMaxWidth()) {
            dayLabels.forEach { label ->
                Text(
                    label,
                    modifier = Modifier.weight(1f),
                    textAlign = TextAlign.Center,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.55f)
                )
            }
        }
    }
}
