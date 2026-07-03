package com.forge.app.ui.goals

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.domain.units.distanceInputValue
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.parseToKm
import com.forge.app.domain.units.parseToLb
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.filterLibrary
import com.forge.app.ui.theme.LocalForgeSettings

/** Step 1 of adding a goal: pick a lift target or one of the custom-goal metrics. */
@Composable
internal fun AddGoalTypeDialog(
    onPickLift: () -> Unit,
    onPickMetric: (GoalMetric) -> Unit,
    onDismiss: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Add a goal") },
        text = {
            Column(Modifier.fillMaxWidth()) {
                GoalTypeOption("Lift target", "Hit a target weight on any exercise", onBg, muted, onPickLift)
                customGoalMetrics.forEach { m ->
                    GoalTypeOption(metricDisplayName(m), metricHint(m), onBg, muted) { onPickMetric(m) }
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}

private fun metricHint(metric: GoalMetric): String = when (metric) {
    GoalMetric.CARDIO_DISTANCE -> "e.g. 5 km this week — tracks your cardio"
    GoalMetric.CARDIO_MINUTES -> "e.g. 90 min this week — tracks your cardio"
    GoalMetric.SESSIONS -> "e.g. train 4× this week"
    GoalMetric.VOLUME -> "total lifted this week or month"
    GoalMetric.BODYWEIGHT -> "reach a target bodyweight, up or down"
}

@Composable
private fun GoalTypeOption(title: String, hint: String, onBg: Color, muted: Color, onClick: () -> Unit) {
    Column(Modifier.fillMaxWidth().clickable(onClick = onClick).padding(vertical = 10.dp)) {
        Text(title, style = MaterialTheme.typography.bodyLarge, color = onBg)
        Text(hint, style = MaterialTheme.typography.labelSmall, color = muted)
    }
}

/**
 * Step 2 for a lift target: a searchable single-select over the whole library. [exclude] drops
 * exercises that already have a goal AND ones you've Hidden in Exercise likes.
 */
@Composable
internal fun LiftTargetPickerDialog(
    exclude: Set<String>,
    onPick: (id: String, name: String) -> Unit,
    onDismiss: () -> Unit
) {
    var query by remember { mutableStateOf("") }
    val results = remember(query, exclude) { filterLibrary(query, exclude) }
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Pick an exercise") },
        text = {
            Column(Modifier.fillMaxWidth()) {
                OutlinedTextField(
                    value = query,
                    onValueChange = { query = it },
                    modifier = Modifier.fillMaxWidth(),
                    placeholder = { Text("Search exercises or muscles") },
                    singleLine = true
                )
                Spacer(Modifier.height(8.dp))
                Column(Modifier.fillMaxWidth().heightIn(max = 320.dp).verticalScroll(rememberScrollState())) {
                    results.forEach { def ->
                        Column(
                            Modifier.fillMaxWidth().clickable { onPick(def.id, def.name) }.padding(vertical = 10.dp)
                        ) {
                            Text(def.name, style = MaterialTheme.typography.bodyLarge, color = onBg)
                            Text(def.muscle.displayName, style = MaterialTheme.typography.labelSmall, color = muted)
                        }
                    }
                    if (results.isEmpty()) {
                        Text(
                            "No matches.",
                            style = MaterialTheme.typography.bodyMedium, color = muted,
                            modifier = Modifier.padding(vertical = 16.dp)
                        )
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}

/** The weight-target dialog for a lift goal (add or edit). */
@Composable
internal fun GoalWeightDialog(
    exerciseName: String,
    currentTargetLb: Double?,
    onSet: (Double) -> Unit,
    onClear: () -> Unit,
    onDismiss: () -> Unit
) {
    val useKg = LocalForgeSettings.current.useKg
    var weightText by remember { mutableStateOf(currentTargetLb?.let { weightInputValue(it, useKg) } ?: "") }
    val weightLb = parseToLb(weightText, useKg)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Goal weight — $exerciseName") },
        text = {
            OutlinedTextField(
                value = weightText,
                onValueChange = { weightText = it.filter { c -> c.isDigit() || c == '.' } },
                label = { Text("Target (${unitLabel(useKg)})") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal)
            )
        },
        confirmButton = {
            TextButton(enabled = weightLb != null && weightLb > 0, onClick = { weightLb?.let(onSet) }) {
                Text("Set goal")
            }
        },
        dismissButton = {
            if (currentTargetLb != null) TextButton(onClick = onClear) { Text("Clear goal") }
            else TextButton(onClick = onDismiss) { Text("Cancel") }
        }
    )
}

/**
 * Step 2 for a custom metric goal: enter a target (in the display unit), pick a period for cumulative
 * metrics, optionally name it. [onConfirm] receives the target already converted to the canonical
 * unit (km / minutes / sessions / lb).
 */
@Composable
internal fun CustomGoalDialog(
    metric: GoalMetric,
    onConfirm: (period: GoalPeriod, targetCanonical: Double, label: String) -> Unit,
    onDismiss: () -> Unit
) {
    val settings = LocalForgeSettings.current
    val decimal = metric != GoalMetric.CARDIO_MINUTES && metric != GoalMetric.SESSIONS
    val unit = customGoalUnitLabel(metric, settings.useKg, settings.useMiles)

    var valueText by remember { mutableStateOf("") }
    var name by remember { mutableStateOf("") }
    var period by remember { mutableStateOf(if (metric == GoalMetric.BODYWEIGHT) GoalPeriod.ALL else GoalPeriod.WEEK) }
    val target = parseCustomTarget(metric, valueText, settings.useKg, settings.useMiles)

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(metricDisplayName(metric)) },
        text = {
            Column(Modifier.fillMaxWidth()) {
                OutlinedTextField(
                    value = valueText,
                    onValueChange = { new -> valueText = new.filter { it.isDigit() || (decimal && it == '.') } },
                    label = { Text("Target ($unit)") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(
                        keyboardType = if (decimal) KeyboardType.Decimal else KeyboardType.Number
                    )
                )
                if (metric.isCumulative) {
                    Spacer(Modifier.height(12.dp))
                    Text("Timeframe", style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Spacer(Modifier.height(6.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        GoalPeriod.entries.forEach { p ->
                            PeriodChip(periodLabel(p), p == period) { period = p }
                        }
                    }
                }
                Spacer(Modifier.height(12.dp))
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text("Name (optional)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
            }
        },
        confirmButton = {
            TextButton(
                enabled = target != null && target > 0,
                onClick = { target?.let { onConfirm(period, it, name.trim()) } }
            ) { Text("Add goal") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}

/** Edit an existing custom goal: change its target or delete it. */
@Composable
internal fun CustomGoalEditDialog(
    goal: ExtendedGoalRepository.Progress,
    onSave: (targetCanonical: Double) -> Unit,
    onDelete: () -> Unit,
    onDismiss: () -> Unit
) {
    val settings = LocalForgeSettings.current
    val decimal = goal.metric != GoalMetric.CARDIO_MINUTES && goal.metric != GoalMetric.SESSIONS
    val unit = customGoalUnitLabel(goal.metric, settings.useKg, settings.useMiles)
    var valueText by remember {
        mutableStateOf(customTargetInputValue(goal.metric, goal.targetValue, settings.useKg, settings.useMiles))
    }
    val target = parseCustomTarget(goal.metric, valueText, settings.useKg, settings.useMiles)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(customGoalTitle(goal)) },
        text = {
            OutlinedTextField(
                value = valueText,
                onValueChange = { new -> valueText = new.filter { it.isDigit() || (decimal && it == '.') } },
                label = { Text("Target ($unit)") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(
                    keyboardType = if (decimal) KeyboardType.Decimal else KeyboardType.Number
                )
            )
        },
        confirmButton = {
            TextButton(enabled = target != null && target > 0, onClick = { target?.let(onSave) }) {
                Text("Save")
            }
        },
        dismissButton = { TextButton(onClick = onDelete) { Text("Delete") } }
    )
}

// ─── Custom-goal unit helpers ─────────────────────────────────────────────

private fun customGoalUnitLabel(metric: GoalMetric, useKg: Boolean, useMiles: Boolean): String = when (metric) {
    GoalMetric.CARDIO_DISTANCE -> distanceUnitLabel(useMiles)
    GoalMetric.CARDIO_MINUTES -> "min"
    GoalMetric.SESSIONS -> "workouts"
    GoalMetric.VOLUME, GoalMetric.BODYWEIGHT -> unitLabel(useKg)
}

/** Parse the target field (display unit) into the metric's canonical unit; null if blank/invalid. */
private fun parseCustomTarget(metric: GoalMetric, text: String, useKg: Boolean, useMiles: Boolean): Double? =
    when (metric) {
        GoalMetric.CARDIO_DISTANCE -> parseToKm(text, useMiles)
        GoalMetric.CARDIO_MINUTES, GoalMetric.SESSIONS -> text.trim().toDoubleOrNull()
        GoalMetric.VOLUME, GoalMetric.BODYWEIGHT -> parseToLb(text, useKg)
    }

/** Inverse of [parseCustomTarget]: canonical value → bare display-unit string for seeding a field. */
private fun customTargetInputValue(metric: GoalMetric, canonical: Double, useKg: Boolean, useMiles: Boolean): String =
    when (metric) {
        GoalMetric.CARDIO_DISTANCE -> distanceInputValue(canonical, useMiles)
        GoalMetric.CARDIO_MINUTES, GoalMetric.SESSIONS -> canonical.toInt().toString()
        GoalMetric.VOLUME, GoalMetric.BODYWEIGHT -> weightInputValue(canonical, useKg)
    }

private fun periodLabel(period: GoalPeriod): String = when (period) {
    GoalPeriod.WEEK -> "This week"
    GoalPeriod.MONTH -> "This month"
    GoalPeriod.ALL -> "All-time"
}

@Composable
private fun PeriodChip(label: String, selected: Boolean, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val bg = MaterialTheme.colorScheme.background
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    Box(
        modifier = Modifier
            .border(1.dp, if (selected) onBg else outline.copy(alpha = 0.4f), RoundedCornerShape(4.dp))
            .background(if (selected) onBg else Color.Transparent, RoundedCornerShape(4.dp))
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 7.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            color = if (selected) bg else muted.copy(alpha = 0.7f)
        )
    }
}
