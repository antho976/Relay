package com.forge.app.ui.common

import com.forge.app.ui.common.window.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
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
        val scheme = MaterialTheme.colorScheme
        AlertDialog(
            onDismissRequest = { viewModel.cancel() },
            containerColor = scheme.surface,
            title = { Text("Workout in progress") },
            text = {
                Text(
                    if (p.loggedSets > 0)
                        "You have a workout in progress with ${p.loggedSets} set" +
                            "${if (p.loggedSets == 1) "" else "s"} logged. Changing your program " +
                            "discards that workout. This can't be undone."
                    else
                        "You have a workout in progress. Changing your program discards it. " +
                            "This can't be undone."
                )
            },
            // The confirm deletes logged sets for good, so it is the error-filled button naming the
            // act, the way every irreversible confirm in Settings reads; it was an accent text
            // button the same weight as Cancel. Keeping the workout is the plain, safe answer.
            confirmButton = {
                Button(
                    onClick = { viewModel.confirm() },
                    colors = ButtonDefaults.buttonColors(containerColor = scheme.error, contentColor = scheme.onError)
                ) { Text("Discard workout") }
            },
            dismissButton = {
                TextButton(onClick = { viewModel.cancel() }) { Text("Keep workout", color = scheme.onSurface) }
            }
        )
    }
}
