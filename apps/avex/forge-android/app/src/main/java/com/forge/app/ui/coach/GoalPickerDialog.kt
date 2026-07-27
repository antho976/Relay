package com.forge.app.ui.coach

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.text.KeyboardOptions
import com.forge.app.domain.coach.BalancePair
import com.forge.app.domain.coach.CoachGoalKind
import com.forge.app.program.MuscleGroup
import com.forge.app.program.Program
import com.forge.app.ui.common.clickableLabeled

/**
 * Add one goal to the portfolio (Coach v3 A2). Two steps in one small dialog: what kind, then its
 * subject and number — a tiny input, which is exactly what a dialog is for (§3).
 *
 * The number is optional on purpose. A goal with no target still gives the coach a priority and a
 * reading ("bench e1RM 212 lb"); demanding a number up front would turn "what am I chasing?" into
 * a form.
 */
@Composable
internal fun GoalPickerDialog(
    onPick: (CoachGoalKind, String, Double?) -> Unit,
    onDismiss: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accentWash = MaterialTheme.colorScheme.primaryContainer

    var kind by remember { mutableStateOf<CoachGoalKind?>(null) }
    var targetKey by remember { mutableStateOf("") }
    var targetText by remember { mutableStateOf("") }

    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = MaterialTheme.colorScheme.surface,
        // Extracted rather than inlined: §4.6's gate reads any `title = { Text(` as a screen naming
        // itself in its chrome, which is the top-bar rule. A dialog IS allowed a title (§3, modal).
        title = { PickerTitle(kind?.displayName ?: "What are you chasing", onBg) },
        text = {
            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
                val chosen = kind
                if (chosen == null) {
                    CoachGoalKind.entries.forEach { k ->
                        PickRow(k.displayName, selected = false, onBg = onBg, wash = accentWash) {
                            kind = k
                            targetKey = ""
                        }
                    }
                } else {
                    when (chosen.scope) {
                        CoachGoalKind.Scope.EXERCISE -> {
                            // §6: the style IS the size; bodySmall is already 12sp.
                            Text(
                                "Which lift",
                                style = MaterialTheme.typography.bodySmall,
                                color = muted
                            )
                            Spacer(Modifier.height(8.dp))
                            Program.days.flatMap { it.exercises }.distinctBy { it.id }.take(24).forEach { plan ->
                                PickRow(plan.name, selected = targetKey == plan.id, onBg = onBg, wash = accentWash) {
                                    targetKey = plan.id
                                }
                            }
                        }
                        CoachGoalKind.Scope.MUSCLE -> {
                            MuscleGroup.entries.forEach { m ->
                                PickRow(m.displayName, selected = targetKey == m.code, onBg = onBg, wash = accentWash) {
                                    targetKey = m.code
                                }
                            }
                        }
                        CoachGoalKind.Scope.BALANCE_PAIR -> {
                            BalancePair.entries.forEach { pair ->
                                val label = if (pair == BalancePair.PUSH_PULL) "Push and pull" else "Quads and hamstrings"
                                PickRow(label, selected = targetKey == pair.code, onBg = onBg, wash = accentWash) {
                                    targetKey = pair.code
                                }
                            }
                        }
                        CoachGoalKind.Scope.NONE -> Unit
                    }
                    if (chosen.scope != CoachGoalKind.Scope.BALANCE_PAIR) {
                        Spacer(Modifier.height(12.dp))
                        OutlinedTextField(
                            value = targetText,
                            onValueChange = { targetText = it.filter { ch -> ch.isDigit() || ch == '.' } },
                            label = { Text("Target (${chosen.unit})") },
                            placeholder = { Text("optional") },
                            singleLine = true,
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                            modifier = Modifier.fillMaxWidth()
                        )
                    }
                }
            }
        },
        confirmButton = {
            val chosen = kind
            val ready = chosen != null &&
                (chosen.scope == CoachGoalKind.Scope.NONE || targetKey.isNotBlank())
            Text(
                "Add",
                style = MaterialTheme.typography.bodyMedium,
                // §5: 0.65 is the hard floor (4.54:1); 0.4 reads as broken rather than as waiting.
                // The control renders passive until it can run, never tappable-but-dead (§2③).
                color = if (ready) MaterialTheme.colorScheme.primary else muted.copy(alpha = 0.65f),
                fontWeight = FontWeight.Medium,
                modifier = Modifier
                    .clip(RoundedCornerShape(50))
                    .then(
                        if (ready) Modifier.clickableLabeled("Add goal") {
                            onPick(chosen!!, targetKey, targetText.toDoubleOrNull())
                            onDismiss()
                        } else Modifier
                    )
                    .padding(horizontal = 12.dp, vertical = 8.dp)
            )
        },
        dismissButton = {
            Text(
                "Cancel",
                style = MaterialTheme.typography.bodyMedium,
                color = muted,
                modifier = Modifier
                    .clip(RoundedCornerShape(50))
                    .clickableLabeled("Cancel") { onDismiss() }
                    .padding(horizontal = 12.dp, vertical = 8.dp)
            )
        }
    )
}

@Composable
private fun PickerTitle(text: String, onBg: Color) {
    Text(text, style = MaterialTheme.typography.titleMedium, color = onBg)
}

@Composable
private fun PickRow(
    label: String,
    selected: Boolean,
    onBg: Color,
    wash: Color,
    onClick: () -> Unit
) {
    Text(
        label,
        style = MaterialTheme.typography.bodyLarge,
        color = onBg,
        fontWeight = if (selected) FontWeight.Medium else FontWeight.Normal,
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(if (selected) wash else Color.Transparent)
            .clickableLabeled(label) { onClick() }
            .padding(horizontal = 14.dp, vertical = 12.dp)
    )
}
