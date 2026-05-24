package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.ui.gym.stats.state.ExerciseYoY
import com.forge.app.ui.gym.stats.state.PeriodComparison

// ─── Period comparison card (#34 week, #130 month) ───────────────────────────

@Composable
fun PeriodComparisonCard(comparison: PeriodComparison, modifier: Modifier = Modifier) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Text(
            "THIS ${comparison.label} VS LAST ${comparison.label}",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontWeight = FontWeight.SemiBold
        )

        // Header row
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("", modifier = Modifier.weight(1.3f)) // label column spacer
            Text("This", style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
            Text("Last", style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
            Text("Δ", style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
        }
        HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.2f))

        CompRow("Sessions", "${comparison.current.sessions}", "${comparison.previous.sessions}",
            comparison.sessionsDelta.let { if (it > 0) "+$it" else "$it" },
            comparison.sessionsDelta > 0)
        CompRow("Volume lb", "${comparison.current.volumeLb.toInt()}", "${comparison.previous.volumeLb.toInt()}",
            comparison.volumeDelta.toInt().let { if (it > 0) "+$it" else "$it" },
            comparison.volumeDelta > 0)
        CompRow("PRs", "${comparison.current.prs}", "${comparison.previous.prs}",
            comparison.prsDelta.let { if (it > 0) "+$it" else "$it" },
            comparison.prsDelta > 0)
    }
}

@Composable
private fun CompRow(label: String, current: String, previous: String, delta: String, positive: Boolean) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1.3f))
        Text(current, style = MaterialTheme.typography.bodySmall, fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.primary, modifier = Modifier.weight(1f))
        Text(previous, style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
        Text(delta, style = MaterialTheme.typography.bodySmall, fontWeight = FontWeight.SemiBold,
            color = if (positive) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.error,
            modifier = Modifier.weight(1f))
    }
}

// ─── Exercise year-over-year (#131) ──────────────────────────────────────────

@Composable
fun ExerciseYoYCard(data: List<ExerciseYoY>, modifier: Modifier = Modifier) {
    if (data.isEmpty()) return
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Text(
            "YEAR OVER YEAR · lb",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontWeight = FontWeight.SemiBold
        )
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("", modifier = Modifier.weight(1.3f))
            Text("This yr", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
            Text("Last yr", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
            Text("Δ", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
        }
        HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.2f))
        data.take(8).forEach { entry ->
            val deltaStr = entry.delta.toInt().let { if (it > 0) "+$it" else "$it" }
            CompRow(
                label = entry.exerciseName,
                current = "${entry.thisYearMaxLb.toInt()}",
                previous = "${entry.lastYearMaxLb.toInt()}",
                delta = deltaStr,
                positive = entry.delta > 0
            )
        }
    }
}
