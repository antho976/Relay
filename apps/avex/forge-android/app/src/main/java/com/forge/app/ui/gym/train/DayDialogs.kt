package com.forge.app.ui.gym.train

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp

@Composable
internal fun DiscardDialog(onConfirm: () -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Discard workout?") },
        text = { Text("You'll lose the sets you've logged this session.") },
        confirmButton = { Button(onClick = onConfirm) { Text("Discard") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Keep going") } }
    )
}

@Composable
internal fun RestTimerSetterDialog(
    exerciseName: String,
    currentSeconds: Int?,
    onSet: (Int) -> Unit,
    onClear: () -> Unit,
    onDismiss: () -> Unit
) {
    val presets = listOf(60 to "1m", 90 to "1:30", 120 to "2m", 150 to "2:30", 180 to "3m", 240 to "4m")
    var selected by remember { mutableStateOf(currentSeconds) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Rest timer — $exerciseName") },
        text = {
            @OptIn(ExperimentalLayoutApi::class)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                presets.forEach { (secs, label) ->
                    FilterChip(selected = selected == secs, onClick = { selected = secs }, label = { Text(label) })
                }
            }
        },
        confirmButton = {
            Button(onClick = { selected?.let { onSet(it) } }, enabled = selected != null) { Text("Save") }
        },
        dismissButton = {
            if (currentSeconds != null) TextButton(onClick = onClear) { Text("Use default") }
            else TextButton(onClick = onDismiss) { Text("Cancel") }
        }
    )
}

@Composable
internal fun GoalSetterDialog(
    exerciseName: String,
    currentGoal: Double?,
    onSet: (Double) -> Unit,
    onClear: () -> Unit,
    onDismiss: () -> Unit
) {
    var weightText by remember { mutableStateOf(currentGoal?.toInt()?.toString() ?: "") }
    val weightLb = weightText.toDoubleOrNull()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Goal weight — $exerciseName") },
        text = {
            OutlinedTextField(
                value = weightText,
                onValueChange = { weightText = it.filter { c -> c.isDigit() || c == '.' } },
                label = { Text("Target weight (lb)") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal)
            )
        },
        confirmButton = {
            Button(onClick = { weightLb?.let { onSet(it) } }, enabled = weightLb != null && weightLb > 0) { Text("Save") }
        },
        dismissButton = {
            if (currentGoal != null) TextButton(onClick = onClear) { Text("Clear goal") }
            else TextButton(onClick = onDismiss) { Text("Cancel") }
        }
    )
}
