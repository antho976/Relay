package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import com.forge.app.ui.common.window.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.domain.coach.BalancePair
import com.forge.app.domain.coach.CoachGoalKind
import com.forge.app.program.MuscleGroup
import com.forge.app.program.Program
import com.forge.app.ui.common.ForgeChoice
import com.forge.app.ui.common.ForgeChoiceList
import com.forge.app.ui.common.ForgeFieldRow
import com.forge.app.ui.common.ForgeGroupSection
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeRowGroup
import com.forge.app.ui.common.ForgeSecondaryCapsule
import com.forge.app.domain.units.filterDecimalInput

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

    var kind by remember { mutableStateOf<CoachGoalKind?>(null) }
    var targetKey by remember { mutableStateOf("") }
    var targetText by remember { mutableStateOf("") }
    val focus = LocalFocusManager.current

    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = MaterialTheme.colorScheme.surface,
        title = {
            Text(
                if (kind == null) "What are you chasing" else kind!!.displayName,
                style = MaterialTheme.typography.titleMedium,
                color = onBg
            )
        },
        text = {
            // Each step is one pick-one group of filled rows ([ForgeChoiceList]); the chosen row
            // lifts out of the group in the accent, the same "picked" as onboarding.
            Column(
                Modifier.fillMaxWidth().verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                val chosen = kind
                if (chosen == null) {
                    ForgeChoiceList(
                        choices = CoachGoalKind.entries.map { ForgeChoice(it.name, it.displayName) },
                        selected = "",
                        onSelect = { key ->
                            kind = CoachGoalKind.entries.first { it.name == key }
                            targetKey = ""
                        }
                    )
                } else {
                    when (chosen.scope) {
                        CoachGoalKind.Scope.EXERCISE -> {
                            ForgeGroupSection("Which lift") {
                                ForgeChoiceList(
                                    choices = Program.days.flatMap { it.exercises }.distinctBy { it.id }.take(24)
                                        .map { ForgeChoice(it.id, it.name) },
                                    selected = targetKey,
                                    onSelect = { targetKey = it }
                                )
                            }
                        }
                        CoachGoalKind.Scope.MUSCLE -> {
                            ForgeChoiceList(
                                choices = MuscleGroup.entries.map { ForgeChoice(it.code, it.displayName) },
                                selected = targetKey,
                                onSelect = { targetKey = it }
                            )
                        }
                        CoachGoalKind.Scope.BALANCE_PAIR -> {
                            ForgeChoiceList(
                                choices = BalancePair.entries.map { pair ->
                                    val label = if (pair == BalancePair.PUSH_PULL) "Push and pull" else "Quads and hamstrings"
                                    ForgeChoice(pair.code, label)
                                },
                                selected = targetKey,
                                onSelect = { targetKey = it }
                            )
                        }
                        CoachGoalKind.Scope.NONE -> Unit
                    }
                    if (chosen.scope != CoachGoalKind.Scope.BALANCE_PAIR) {
                        ForgeRowGroup({
                            ForgeFieldRow(
                                label = "Target",
                                value = targetText,
                                onValueChange = { targetText = filterDecimalInput(it) },
                                placeholder = "Optional",
                                suffix = chosen.unit,
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done),
                                keyboardActions = KeyboardActions(onDone = { focus.clearFocus() })
                            )
                        })
                    }
                }
            }
        },
        confirmButton = {
            val chosen = kind
            val ready = chosen != null &&
                (chosen.scope == CoachGoalKind.Scope.NONE || targetKey.isNotBlank())
            ForgePrimaryCapsule(
                "Add",
                onClick = {
                    if (chosen != null) {
                        onPick(chosen, targetKey, targetText.toDoubleOrNull())
                        onDismiss()
                    }
                },
                enabled = ready
            )
        },
        dismissButton = {
            ForgeSecondaryCapsule("Cancel", onClick = onDismiss)
        }
    )
}
