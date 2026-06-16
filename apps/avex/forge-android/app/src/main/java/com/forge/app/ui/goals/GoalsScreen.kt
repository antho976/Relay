package com.forge.app.ui.goals

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.GoalRepository
import com.forge.app.domain.units.parseToLb
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.EmptyState
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.emphasized

/** A goal being added (currentTargetLb = null) or edited (existing target). */
private data class Editing(val exerciseId: String, val name: String, val currentTargetLb: Double?)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GoalsScreen(
    onBack: () -> Unit,
    viewModel: GoalsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var pickerOpen by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<Editing?>(null) }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Goals.", style = MaterialTheme.typography.headlineMedium) },
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        when {
            state.loading -> Box(Modifier.fillMaxSize().padding(inner), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            else -> Column(
                Modifier.fillMaxSize().padding(inner).verticalScroll(rememberScrollState()).padding(horizontal = 24.dp)
            ) {
                Spacer(Modifier.height(8.dp))
                Text(
                    "Set a target weight for a lift and track your way to it. Progress is your heaviest set so far.",
                    style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
                )
                Spacer(Modifier.height(16.dp))

                if (state.goals.isEmpty()) {
                    EmptyState(
                        emoji = "🎯",
                        title = "No goals yet.",
                        subtitle = "Add one below — or set a goal on any exercise during a workout."
                    )
                } else {
                    state.goals.forEach { g ->
                        GoalRow(g, onBg, muted, accent, outline) {
                            editing = Editing(g.exerciseId, g.name, g.targetLb)
                        }
                        Spacer(Modifier.height(16.dp))
                    }
                }

                Spacer(Modifier.height(8.dp))
                val canAdd = state.addable.isNotEmpty()
                Text(
                    if (canAdd) "+ Add a goal" else "Every program exercise already has a goal",
                    style = MaterialTheme.typography.labelLarge,
                    color = if (canAdd) accent else muted,
                    modifier = Modifier
                        .let { if (canAdd) it.clickable { pickerOpen = true } else it }
                        .padding(vertical = 8.dp)
                )
                Spacer(Modifier.height(40.dp))
            }
        }
    }

    if (pickerOpen) {
        ExercisePickerDialog(
            options = state.addable,
            onPick = { opt -> pickerOpen = false; editing = Editing(opt.id, opt.name, null) },
            onDismiss = { pickerOpen = false }
        )
    }

    editing?.let { e ->
        GoalWeightDialog(
            exerciseName = e.name,
            currentTargetLb = e.currentTargetLb,
            onSet = { lb -> viewModel.setGoal(e.exerciseId, lb); editing = null },
            onClear = { viewModel.clearGoal(e.exerciseId); editing = null },
            onDismiss = { editing = null }
        )
    }
}

@Composable
private fun GoalRow(
    g: GoalRepository.GoalProgress,
    onBg: Color, muted: Color, accent: Color, outline: Color,
    onClick: () -> Unit
) {
    val useKg = LocalForgeSettings.current.useKg
    Column(Modifier.fillMaxWidth().clickable(onClick = onClick)) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(g.name, style = MaterialTheme.typography.bodyLarge, color = emphasized(onBg))
            if (g.achieved) {
                Text("reached ✓", style = MaterialTheme.typography.labelMedium, color = accent)
            } else {
                Text("${(g.fraction * 100).toInt()}%", style = MaterialTheme.typography.labelMedium, color = muted)
            }
        }
        Spacer(Modifier.height(6.dp))
        ProgressBar(g.fraction, if (g.achieved) accent else onBg, outline)
        Spacer(Modifier.height(6.dp))
        Text(
            // weightInputValue gives the bare number in the display unit; the unit is appended once
            // (formatWeight already includes it, which doubled to "200 lb lb").
            "${weightInputValue(g.currentBestLb, useKg)} / ${weightInputValue(g.targetLb, useKg)} ${unitLabel(useKg)}",
            style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp
        )
    }
}

@Composable
private fun ProgressBar(fraction: Float, fill: Color, outline: Color) {
    Box(
        Modifier.fillMaxWidth().height(4.dp).clip(RoundedCornerShape(2.dp)).background(outline.copy(alpha = 0.25f))
    ) {
        Box(
            Modifier.fillMaxWidth(fraction.coerceIn(0f, 1f)).height(4.dp)
                .clip(RoundedCornerShape(2.dp)).background(fill)
        )
    }
}

@Composable
private fun ExercisePickerDialog(
    options: List<GoalsViewModel.ExerciseOption>,
    onPick: (GoalsViewModel.ExerciseOption) -> Unit,
    onDismiss: () -> Unit
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Pick an exercise") },
        text = {
            Column(Modifier.fillMaxWidth().heightIn(max = 360.dp).verticalScroll(rememberScrollState())) {
                options.forEach { opt ->
                    Text(
                        opt.name,
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onBackground,
                        modifier = Modifier.fillMaxWidth().clickable { onPick(opt) }.padding(vertical = 12.dp)
                    )
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}

@Composable
private fun GoalWeightDialog(
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
            TextButton(
                enabled = weightLb != null && weightLb > 0,
                onClick = { weightLb?.let(onSet) }
            ) { Text("Set goal") }
        },
        dismissButton = {
            if (currentTargetLb != null) TextButton(onClick = onClear) { Text("Clear goal") }
            else TextButton(onClick = onDismiss) { Text("Cancel") }
        }
    )
}
