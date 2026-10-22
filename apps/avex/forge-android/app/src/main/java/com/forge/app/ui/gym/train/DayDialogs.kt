package com.forge.app.ui.gym.train

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.text.KeyboardOptions
import com.forge.app.ui.common.window.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp

/**
 * [nothingLogged]: the session holds nothing the user authored (FINISH on an empty workout lands
 * here). There is nothing to resume then, and "Resume later" would keep an empty session active that
 * every other day then offers to discard, so only Discard and Keep going are offered.
 */
@Composable
internal fun LeaveSessionDialog(
    onResumeLater: () -> Unit,
    onDiscard: () -> Unit,
    onKeepGoing: () -> Unit,
    nothingLogged: Boolean = false
) {
    if (nothingLogged) {
        AlertDialog(
            onDismissRequest = onKeepGoing,
            title = { Text("Nothing logged yet") },
            text = { Text("This workout has no sets to save. Keep going, or discard it.") },
            confirmButton = { DialogPrimary("Keep going", onClick = onKeepGoing) },
            dismissButton = { DialogQuiet("Discard workout", onClick = onDiscard) }
        )
        return
    }
    AlertDialog(
        onDismissRequest = onKeepGoing,
        title = { Text("Leave this workout?") },
        text = { Text("Your logged sets are saved. Resume this workout later from its day, or discard what you've logged.") },
        confirmButton = { DialogPrimary("Resume later", onClick = onResumeLater) },
        dismissButton = {
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                DialogQuiet("Discard", onClick = onDiscard)
                DialogQuiet("Keep going", onClick = onKeepGoing)
            }
        }
    )
}

/**
 * FINISH while some exercises were never started. Finishing can't be undone, so this is one of the
 * few places §12 allows a dialog: it names what stays unlogged, and the safe answer is the primary.
 */
@Composable
internal fun FinishUntouchedDialog(
    untouched: List<String>,
    onFinish: () -> Unit,
    onKeepGoing: () -> Unit
) {
    val count = untouched.size
    val names = when {
        count == 1 -> untouched.first()
        count == 2 -> "${untouched[0]} and ${untouched[1]}"
        else -> "${untouched[0]}, ${untouched[1]} and ${count - 2} more"
    }
    AlertDialog(
        onDismissRequest = onKeepGoing,
        title = { Text(if (count == 1) "1 exercise not started" else "$count exercises not started") },
        text = { Text("$names ${if (count == 1) "has" else "have"} no sets. Finishing closes the workout with ${if (count == 1) "it" else "them"} unlogged.") },
        confirmButton = { DialogPrimary("Keep going", onClick = onKeepGoing) },
        dismissButton = { DialogQuiet("Finish anyway", onClick = onFinish) }
    )
}

/**
 * Shown when this day is opened while a DIFFERENT day's workout is still in progress (only one
 * session is active at a time). Resume the other one, or discard it and start this day.
 */
@Composable
internal fun CrossDaySessionDialog(
    otherDayName: String,
    thisDayName: String,
    onDiscardAndStart: () -> Unit,
    onGoBack: () -> Unit
) {
    AlertDialog(
        onDismissRequest = onGoBack,
        title = { Text("$otherDayName in progress") },
        text = { Text("You have an unfinished $otherDayName workout. Go back to resume it, or discard it and start $thisDayName instead.") },
        confirmButton = { DialogPrimary("Resume $otherDayName", onClick = onGoBack) },
        dismissButton = { DialogQuiet("Discard & start $thisDayName", onClick = onDiscardAndStart) }
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
    val weightUnit = com.forge.app.ui.theme.LocalForgeSettings.current.weightUnit
    var weightText by remember { mutableStateOf(currentGoal?.let { com.forge.app.domain.units.weightInputValue(it, weightUnit) } ?: "") }
    val weightLb = com.forge.app.domain.units.parseToLb(weightText, weightUnit)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Goal for $exerciseName") },
        text = {
            OutlinedTextField(
                value = weightText,
                onValueChange = { weightText = it.filter { c -> c.isDigit() || c == '.' } },
                label = { Text("Target weight (${com.forge.app.domain.units.unitLabel(weightUnit)})") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal)
            )
        },
        confirmButton = {
            DialogPrimary("Save", enabled = weightLb != null && weightLb > 0) { weightLb?.let { onSet(it) } }
        },
        dismissButton = {
            if (currentGoal != null) DialogQuiet("Clear goal", onClick = onClear)
            else DialogQuiet("Cancel", onClick = onDismiss)
        }
    )
}

/** A session dialog's action: the red, square-cornered primary the rest of the app uses. */
@Composable
private fun DialogPrimary(text: String, enabled: Boolean = true, onClick: () -> Unit) {
    Button(onClick = onClick, enabled = enabled, shape = androidx.compose.foundation.shape.RoundedCornerShape(4.dp)) { Text(text) }
}

/** A session dialog's other way out: outlined and square-cornered beside the primary, never a pill. */
@Composable
private fun DialogQuiet(text: String, onClick: () -> Unit) {
    androidx.compose.material3.OutlinedButton(
        onClick = onClick,
        shape = androidx.compose.foundation.shape.RoundedCornerShape(4.dp),
        border = androidx.compose.foundation.BorderStroke(1.dp, com.forge.app.ui.theme.OpenColors.Rule),
        colors = androidx.compose.material3.ButtonDefaults.outlinedButtonColors(contentColor = androidx.compose.material3.MaterialTheme.colorScheme.onSurface)
    ) { Text(text) }
}
