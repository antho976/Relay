package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.stats.state.WeeklyEffortCounts

private val ColorEasy = Color(0xFF4CAF50)
private val ColorJustRight = Color(0xFF2196F3)
private val ColorHard = Color(0xFFFF9800)
private val ColorBrutal = Color(0xFFF44336)

@Composable
fun EffortDistributionCard(weeks: List<WeeklyEffortCounts>, modifier: Modifier = Modifier) {
    if (weeks.isEmpty()) return
    val maxTotal = weeks.maxOf { it.total }.coerceAtLeast(1)

    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Text(
            "EFFORT DISTRIBUTION · 8 WEEKS",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontWeight = FontWeight.SemiBold
        )
        weeks.forEach { week ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Text(
                    week.weekLabel,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.weight(0.12f)
                )
                Row(
                    modifier = Modifier.weight(1f).height(14.dp).clip(RoundedCornerShape(4.dp)),
                    horizontalArrangement = Arrangement.Start
                ) {
                    StackedSegment(week.easy, maxTotal, ColorEasy)
                    StackedSegment(week.justRight, maxTotal, ColorJustRight)
                    StackedSegment(week.hard, maxTotal, ColorHard)
                    StackedSegment(week.brutal, maxTotal, ColorBrutal)
                    val remaining = maxTotal - week.total
                    if (remaining > 0) StackedSegment(remaining, maxTotal, MaterialTheme.colorScheme.outline.copy(alpha = 0.2f))
                }
                Text(
                    "${week.total}",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 10.sp,
                    modifier = Modifier.weight(0.08f)
                )
            }
        }
        // Legend
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            LegendDot("Easy", ColorEasy)
            LegendDot("Good", ColorJustRight)
            LegendDot("Hard", ColorHard)
            LegendDot("Brutal", ColorBrutal)
        }
    }
}

@Composable
private fun RowScope.StackedSegment(count: Int, max: Int, color: Color) {
    if (count <= 0) return
    Box(
        modifier = Modifier
            .weight(count.toFloat() / max)
            .height(14.dp)
            .background(color)
    )
}

@Composable
private fun LegendDot(label: String, color: Color) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        Box(Modifier.height(8.dp).background(color, RoundedCornerShape(2.dp)).padding(horizontal = 4.dp))
        Text(label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}
