package com.forge.app.ui.gym.stats

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.program.MuscleGroup
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun PrsContent(
    modifier: Modifier = Modifier,
    viewModel: PrsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var showTargetDialog by remember { mutableStateOf(false) }

    LazyColumn(
        modifier = modifier,
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        // Monthly PR target card (#84)
        item("monthly-target") {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
                    .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Column {
                        Text(
                            "THIS MONTH",
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            fontWeight = FontWeight.SemiBold
                        )
                        Text(
                            "${state.monthlyCount} PR${if (state.monthlyCount != 1) "s" else ""}",
                            style = MaterialTheme.typography.headlineMedium,
                            color = MaterialTheme.colorScheme.onSurface,
                            fontWeight = FontWeight.Black
                        )
                        if (state.monthlyTarget > 0) {
                            Text(
                                "Goal: ${state.monthlyTarget}",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }
                    IconButton(onClick = { showTargetDialog = true }) {
                        Icon(Icons.Default.Edit, contentDescription = "Set PR goal")
                    }
                }
                if (state.monthlyTarget > 0) {
                    LinearProgressIndicator(
                        progress = { (state.monthlyCount.toFloat() / state.monthlyTarget).coerceIn(0f, 1f) },
                        modifier = Modifier.fillMaxWidth(),
                        color = if (state.monthlyCount >= state.monthlyTarget)
                            MaterialTheme.colorScheme.tertiary
                        else MaterialTheme.colorScheme.primary
                    )
                }
            }
        }

        // Period filter chips
        item("period-filter") {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                PrPeriodFilter.entries.forEach { period ->
                    FilterChip(
                        selected = state.periodFilter == period,
                        onClick = { viewModel.setPeriodFilter(period) },
                        label = { Text(period.label) }
                    )
                }
            }
        }

        // Muscle group filter chips
        item("muscle-filter") {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = state.muscleFilter == null,
                    onClick = { viewModel.setMuscleFilter(null) },
                    label = { Text("All muscles") }
                )
                MuscleGroup.entries.forEach { muscle ->
                    FilterChip(
                        selected = state.muscleFilter == muscle,
                        onClick = { viewModel.setMuscleFilter(if (state.muscleFilter == muscle) null else muscle) },
                        label = { Text(muscle.displayName) }
                    )
                }
            }
        }

        item("count") {
            Text(
                "${state.filtered.size} PRs",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }

        if (state.filtered.isEmpty()) {
            item("empty") {
                Text(
                    "No PRs match these filters.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(vertical = 24.dp)
                )
            }
        } else {
            items(state.filtered, key = { "${it.exerciseId}_${it.sessionDate}" }) { row ->
                PrRow(row)
                HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.15f))
            }
        }
    }

    if (showTargetDialog) {
        MonthlyTargetDialog(
            current = state.monthlyTarget,
            onDismiss = { showTargetDialog = false },
            onConfirm = { target ->
                viewModel.setMonthlyTarget(target)
                showTargetDialog = false
            }
        )
    }
}

@Composable
private fun PrRow(row: PrDisplayRow) {
    val zone = ZoneId.systemDefault()
    val fmt = DateTimeFormatter.ofPattern("MMM d")
    val dateStr = Instant.ofEpochMilli(row.sessionDate).atZone(zone).format(fmt)
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(row.exerciseName, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium)
            row.muscle?.let {
                Text(it.displayName, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        Text(dateStr, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun MonthlyTargetDialog(current: Int, onDismiss: () -> Unit, onConfirm: (Int) -> Unit) {
    var text by remember { mutableStateOf(if (current > 0) current.toString() else "") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Monthly PR goal") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("How many PRs do you want to hit this month?", style = MaterialTheme.typography.bodyMedium)
                TextField(
                    value = text,
                    onValueChange = { if (it.length <= 2 && it.all { c -> c.isDigit() }) text = it },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    singleLine = true,
                    placeholder = { Text("e.g. 5") }
                )
            }
        },
        confirmButton = {
            TextButton(onClick = { onConfirm(text.toIntOrNull() ?: 0) }) { Text("Save") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        }
    )
}
