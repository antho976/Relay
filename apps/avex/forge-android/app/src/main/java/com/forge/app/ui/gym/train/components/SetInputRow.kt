package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.LoggedSet

/**
 * Weight + reps inputs + Log set button for an expanded exercise card.
 *
 * Underline-style: values render in the headline serif type with a thin underline,
 * matching the editorial design (no filled / outlined text-field boxes).
 *
 * Forgiving input (#20): typing "45x10" in the weight field auto-splits into weight=45 reps=10.
 * Use-last chip (#21), weight suggestion (#13/#12) and PR hint (#100) work as before.
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

    val prRepsHint = remember(weight, priorSets) {
        val weightLb = weight.trim().toDoubleOrNull() ?: return@remember null
        repsNeededForPr(priorSets, weightLb)
    }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    Column(modifier = modifier.fillMaxWidth()) {
        // Auto-progression suggestion (#13/#12)
        if (suggestedWeight != null) {
            Row(
                modifier = Modifier.fillMaxWidth().padding(bottom = 2.dp),
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

        // Labels
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.Bottom
        ) {
            Text(
                "WEIGHT · LB",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                modifier = Modifier.weight(1f)
            )
            Text(
                "REPS",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                modifier = Modifier.weight(0.7f)
            )
        }

        // Inputs row
        Row(
            modifier = Modifier.fillMaxWidth().padding(top = 2.dp),
            verticalAlignment = Alignment.Bottom,
            horizontalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            UnderlineNumberField(
                value = weight,
                onValueChange = ::onWeightChange,
                placeholder = "0",
                keyboardType = KeyboardType.Text,
                imeAction = ImeAction.Next,
                modifier = Modifier.weight(1f)
            )
            UnderlineNumberField(
                value = reps,
                onValueChange = { new -> if (new.all { it.isDigit() }) reps = new },
                placeholder = "0",
                keyboardType = KeyboardType.Number,
                imeAction = ImeAction.Done,
                supportingText = prRepsHint?.let { "$it for PR" },
                modifier = Modifier.weight(0.7f)
            )
            Button(
                onClick = {
                    val r = reps.toIntOrNull() ?: return@Button
                    onSubmit(weight.trim(), r)
                    reps = ""
                },
                enabled = canSubmit,
                shape = androidx.compose.foundation.shape.RoundedCornerShape(50),
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = onBg,
                    contentColor = MaterialTheme.colorScheme.background,
                    disabledContainerColor = onBg.copy(alpha = 0.35f),
                    disabledContentColor = MaterialTheme.colorScheme.background.copy(alpha = 0.7f)
                )
            ) {
                Text("Log set →", style = MaterialTheme.typography.labelSmall)
            }
        }

        // Use-last chip (#21)
        if (prefillWeight != null && weight.isBlank()) {
            TextButton(
                onClick = { weight = prefillWeight },
                contentPadding = PaddingValues(horizontal = 4.dp, vertical = 0.dp),
                modifier = Modifier.height(28.dp)
            ) {
                Text(
                    "Use last: $prefillWeight",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.primary,
                    fontStyle = FontStyle.Italic
                )
            }
        }
    }
}

@Composable
private fun UnderlineNumberField(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    keyboardType: KeyboardType,
    imeAction: ImeAction,
    modifier: Modifier = Modifier,
    supportingText: String? = null
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    Column(modifier = modifier) {
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            textStyle = MaterialTheme.typography.headlineMedium.copy(color = onBg),
            singleLine = true,
            cursorBrush = SolidColor(accent),
            keyboardOptions = KeyboardOptions(
                keyboardType = keyboardType,
                imeAction = imeAction
            ),
            decorationBox = { inner ->
                Box {
                    if (value.isEmpty()) {
                        Text(
                            placeholder,
                            style = MaterialTheme.typography.headlineMedium,
                            color = muted.copy(alpha = 0.4f)
                        )
                    }
                    inner()
                }
            }
        )
        HorizontalDivider(
            modifier = Modifier.padding(top = 2.dp),
            thickness = 1.dp,
            color = outline.copy(alpha = 0.5f)
        )
        if (supportingText != null) {
            Text(
                supportingText,
                style = MaterialTheme.typography.labelSmall,
                color = accent,
                modifier = Modifier.padding(top = 2.dp)
            )
        }
    }
}

/**
 * Returns the rep count needed to beat the all-time PR at [weightLb], or null when no PR is reachable.
 * Mirrors PrDetector.isPr: at fixed weight, PR requires more reps than the historical max at this weight or above.
 */
private fun repsNeededForPr(history: List<LoggedSet>, weightLb: Double): Int? {
    val maxRepsAtOrAbove = history
        .filter { it.weightLb != null && it.weightLb >= weightLb }
        .maxOfOrNull { it.reps }
    return maxRepsAtOrAbove?.let { it + 1 }
}
