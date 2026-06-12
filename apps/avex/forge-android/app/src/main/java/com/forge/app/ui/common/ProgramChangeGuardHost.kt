package com.forge.app.ui.common

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.hilt.navigation.compose.hiltViewModel

/**
 * Root-level host for the [ProgramChangeGuard] confirm dialog. Rendered once over the whole app so a
 * "discard your workout?" prompt appears in place — wherever the program change was triggered (the
 * Overview deload card, the Week Brief, Settings, or a day re-roll) — instead of sending the user
 * off to find the workout elsewhere.
 */
@Composable
fun ProgramChangeGuardHost(
    viewModel: ProgramChangeGuardViewModel = hiltViewModel()
) {
    val pending by viewModel.guard.pending.collectAsState()
    pending?.let { p ->
        AlertDialog(
            onDismissRequest = { viewModel.cancel() },
            title = { Text("Workout in progress") },
            text = {
                Text(
                    if (p.loggedSets > 0)
                        "You have a workout in progress with ${p.loggedSets} set" +
                            "${if (p.loggedSets == 1) "" else "s"} logged. Changing your program " +
                            "discards that workout — this can't be undone."
                    else
                        "You have a workout in progress. Changing your program discards it — " +
                            "this can't be undone."
                )
            },
            confirmButton = {
                TextButton(onClick = { viewModel.confirm() }) { Text("Discard & continue") }
            },
            dismissButton = {
                TextButton(onClick = { viewModel.cancel() }) { Text("Cancel") }
            }
        )
    }
}
