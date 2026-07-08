package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * Shown right after a "Make default" swap: offers to dislike the swapped-out [exerciseName] so the
 * generator and swap picker never suggest it again. The two text buttons mute the prompt itself —
 * [onNotThisWorkout] silences it for the rest of this session, [onNeverAsk] turns it off for good
 * (re-enabled from Settings → Exercise preferences).
 */
@Composable
fun DislikeSwapPromptDialog(
    exerciseName: String,
    onDislike: () -> Unit,
    onKeep: () -> Unit,
    onNotThisWorkout: () -> Unit,
    onNeverAsk: () -> Unit
) {
    AlertDialog(
        onDismissRequest = onKeep,
        title = { Text("Hide $exerciseName?") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    "You swapped it out. Dislike it and Avex won't suggest it in swaps or new programs.",
                    style = MaterialTheme.typography.bodyMedium
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TextButton(onClick = onNotThisWorkout, modifier = Modifier.weight(1f)) {
                        Text("Not this workout", style = MaterialTheme.typography.labelSmall)
                    }
                    TextButton(onClick = onNeverAsk, modifier = Modifier.weight(1f)) {
                        Text("Never ask", style = MaterialTheme.typography.labelSmall)
                    }
                }
            }
        },
        confirmButton = { Button(onClick = onDislike) { Text("Hide") } },
        dismissButton = { TextButton(onClick = onKeep) { Text("Keep it") } }
    )
}
