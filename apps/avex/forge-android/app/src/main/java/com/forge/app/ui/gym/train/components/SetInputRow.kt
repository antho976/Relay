package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.LoggedSet

/**
 * Weight + reps inputs + Add button for an expanded exercise card.
 *
 * Forgiving input (#20): typing "45x10" (or "45 X 10") in the weight field auto-splits into
 * weight=45 and reps=10 so users can log a set from a single keystroke sequence.
 *
 * Use-last chip (#21): when the weight field is cleared and [prefillWeight] is known, a tap-to-fill
 * chip appears so the user can restore the last-session value in one tap.
 *
 * Weight suggestion (#13/#12): when [suggestedWeight] is non-null (computed by the VM from last
 * performance + difficulty rating + rep-range progression), a "Try: X lb" hint row is shown with a
 * one-tap "Use" button.
 *
 * PR hint (#100): when [priorSets] is non-empty and the current weight has a numeric value,
 * shows "Hit N reps for a PR" next to the reps field based on history at this weight or higher.
 */
@Composable
fun SetInputRow(
    prefillWeight: String?,
    suggestedWeight: String? = null,
    suggestionReason: String? = null,
    priorSets: List<LoggedSet> = emptyList(),
    onSubmit: (weightText: String, reps: Int) -> Unit,
    modifier: Modifier = Modifier
) {
    var weight by rememberSaveable(prefillWeight) { mutableStateOf(prefillWeight.orEmpty()) }
    var reps by rememberSaveable { mutableStateOf("") }

    fun onWeightChange(new: String) {
        val match = Regex("""^([0-9]*\.?[0-9]+)\s*[xX]\s*([0-9]+)$""").matchEntire(new.trim())
        if (match != null) {
            weight = match.groupValues[1]
            reps = match.groupValues[2]
        } else {
            weight = new
        }
    }

    val canSubmit = remember(weight, reps) {
        weight.isNotBlank() && reps.toIntOrNull()?.let { it > 0 } == true
    }

    // PR hint: how many reps at this weight to beat all-time best (#100)
    val prRepsHint = remember(weight, priorSets) {
        val weightLb = weight.trim().toDoubleOrNull() ?: return@remember null
        repsNeededForPr(priorSets, weightLb)
    }

    Column(modifier = modifier.fillMaxWidth()) {
        // #13/#12: Auto-progression suggestion
        if (suggestedWeight != null) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(bottom = 2.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                val hint = buildString {
                    append("Try: $suggestedWeight lb")
                    if (!suggestionReason.isNullOrBlank()) append(" · $suggestionReason")
                }
                Text(
                    hint,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.primary.copy(alpha = 0.75f),
                    modifier = Modifier.weight(1f)
                )
                TextButton(
                    onClick = { weight = suggestedWeight },
                    contentPadding = PaddingValues(horizontal = 6.dp, vertical = 0.dp),
                    modifier = Modifier.height(24.dp)
                ) {
                    Text("Use", style = MaterialTheme.typography.labelSmall)
                }
            }
        }

        // Main input row
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            OutlinedTextField(
                value = weight,
                onValueChange = ::onWeightChange,
                modifier = Modifier.weight(1.2f),
                label = { Text("Weight") },
                placeholder = { Text("e.g. 25 or 45x10") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(
                    keyboardType = KeyboardType.Text,
                    imeAction = ImeAction.Next
                )
            )
            OutlinedTextField(
                value = reps,
                onValueChange = { new -> if (new.all { it.isDigit() }) reps = new },
                modifier = Modifier.weight(0.8f),
                label = { Text("Reps") },
                supportingText = prRepsHint?.let { needed ->
                    { Text("$needed for PR", style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.primary) }
                },
                singleLine = true,
                keyboardOptions = KeyboardOptions(
                    keyboardType = KeyboardType.Number,
                    imeAction = ImeAction.Done
                )
            )
            Button(
                onClick = {
                    val r = reps.toIntOrNull() ?: return@Button
                    onSubmit(weight.trim(), r)
                    reps = ""
                },
                enabled = canSubmit,
                modifier = Modifier.height(56.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = MaterialTheme.colorScheme.primary,
                    contentColor = MaterialTheme.colorScheme.onPrimary
                )
            ) {
                Text("Add")
            }
        }

        // #21: Use-last chip — shown when weight is blank and a last-session weight exists
        if (prefillWeight != null && weight.isBlank()) {
            TextButton(
                onClick = { weight = prefillWeight },
                contentPadding = PaddingValues(horizontal = 4.dp, vertical = 0.dp),
                modifier = Modifier.height(28.dp)
            ) {
                Text(
                    "Use last: $prefillWeight",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.primary
                )
            }
        }
    }
}

/**
 * Returns the rep count needed to beat the all-time PR at [weightLb], or null when:
 * - history is empty at or above this weight (the weight itself would be a PR at any reps)
 * - [weightLb] is heavier than any historical set (first set at this weight is always a PR)
 *
 * Logic mirrors [PrDetector.isPr]: a PR requires being strictly heavier than the max weight
 * logged at the same-or-higher rep count. At a fixed weight, that means logging more reps
 * than the historical max at this weight or above.
 */
private fun repsNeededForPr(history: List<LoggedSet>, weightLb: Double): Int? {
    val maxRepsAtOrAbove = history
        .filter { it.weightLb != null && it.weightLb >= weightLb }
        .maxOfOrNull { it.reps }
    return maxRepsAtOrAbove?.let { it + 1 }
}
