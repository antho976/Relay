package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.ui.gym.stats.state.HeatmapCell
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.format.TextStyle
import java.util.Locale

/**
 * 7×7 grid of the most recent 49 days. Cell intensity scales with that day's exercise count.
 * Cells are arranged row-by-row (row = week), oldest in the top-left → newest in the bottom-right.
 * Column headers show the day-of-week letter for the starting weekday; today's cell is ringed.
 */
@Composable
fun FrequencyHeatmap(cells: List<HeatmapCell>, modifier: Modifier = Modifier) {
    val accent = MaterialTheme.colorScheme.primary
    val maxCount = (cells.maxOfOrNull { it.count } ?: 0).coerceAtLeast(1)
    val today = remember { LocalDate.now() }

    // Column headers derived from the day-of-week of the oldest cell
    val startDow: DayOfWeek = cells.firstOrNull()?.date?.dayOfWeek ?: DayOfWeek.MONDAY
    val colLabels = (0 until COLS).map { i ->
        DayOfWeek.of(((startDow.value - 1 + i) % 7) + 1)
            .getDisplayName(TextStyle.NARROW, Locale.getDefault())
    }

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
                "LAST 7 WEEKS",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontWeight = FontWeight.SemiBold
            )

            // Day-of-week column headers
            Row(horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                colLabels.forEach { label ->
                    Text(
                        label,
                        modifier = Modifier.size(24.dp),
                        textAlign = TextAlign.Center,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
                    )
                }
            }

            // Heatmap grid
            Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                for (row in 0 until ROWS) {
                    Row(horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                        for (col in 0 until COLS) {
                            val index = row * COLS + col
                            val cell = cells.getOrNull(index)
                            HeatmapCellView(
                                count = cell?.count ?: 0,
                                maxCount = maxCount,
                                accent = accent,
                                isToday = cell?.date == today
                            )
                        }
                    }
                }
            }

            // Legend strip
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                Text(
                    "Less",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f)
                )
                Spacer(Modifier.width(2.dp))
                listOf(0.08f, 0.30f, 0.55f, 0.78f, 1.0f).forEach { alpha ->
                    Box(
                        Modifier
                            .size(12.dp)
                            .clip(RoundedCornerShape(2.dp))
                            .background(accent.copy(alpha = alpha))
                    )
                }
                Spacer(Modifier.width(2.dp))
                Text(
                    "More",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f)
                )
            }
        }
    }
}

@Composable
private fun HeatmapCellView(
    count: Int,
    maxCount: Int,
    accent: androidx.compose.ui.graphics.Color,
    isToday: Boolean = false
) {
    val alpha = when {
        count == 0 -> 0.08f
        else -> 0.2f + 0.8f * (count.toFloat() / maxCount).coerceIn(0f, 1f)
    }
    val base = Modifier
        .size(24.dp)
        .clip(RoundedCornerShape(3.dp))
        .background(accent.copy(alpha = alpha))
    Box(
        if (isToday) base.border(1.5.dp, accent, RoundedCornerShape(3.dp)) else base
    )
}

private const val ROWS = 7
private const val COLS = 7
