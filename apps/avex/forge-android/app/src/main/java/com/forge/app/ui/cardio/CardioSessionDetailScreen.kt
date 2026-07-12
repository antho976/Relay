package com.forge.app.ui.cardio

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.LaunchedEffect
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.cardio.components.CardioLogSheet
import com.forge.app.ui.cardio.components.CardioSessionDetailSheet

/**
 * Routed, single-session cardio stats — what a cardio row in the History page opens. Shows the same
 * [CardioSessionDetailSheet] as the in-tab overlay, with Edit (the log sheet) and a confirmed Delete.
 */
@Composable
fun CardioSessionDetailScreen(
    onBack: () -> Unit,
    viewModel: CardioSessionDetailViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var confirmDelete by remember { mutableStateOf(false) }
    var leaving by remember { mutableStateOf(false) }

    // Health Connect's per-route consent screen — hands back the chosen session's GPS track (or null).
    val routeLauncher = rememberLauncherForActivityResult(
        contract = androidx.health.connect.client.contracts.ExerciseRouteRequestContract()
    ) { route -> viewModel.onRouteConsented(route) }

    // Re-read the steps/GPS grant + day data on resume — the user may connect in Settings and return.
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    androidx.compose.runtime.DisposableEffect(lifecycleOwner) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME) viewModel.loadWearable()
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    // Leave exactly once — when the entry is deleted, or once loaded and it no longer exists. A delete
    // makes BOTH true (deleted flips, and the DB flow drops the row), so the `leaving` guard stops a
    // second onBack() that would pop past this screen (e.g. the History list beneath it).
    LaunchedEffect(state.deleted, state.loaded, state.entry) {
        if (!leaving && (state.deleted || (state.loaded && state.entry == null))) {
            leaving = true
            onBack()
        }
    }

    val entry = state.entry ?: return  // still loading, or already gone (handled above)

    if (state.editing) {
        CardioLogSheet(
            onDismiss = viewModel::closeEdit,
            onSave = viewModel::save,
            onCreateCustom = viewModel::addCustomType,
            editing = entry,
            useMiles = state.useMiles
        )
    } else {
        CardioSessionDetailSheet(
            entry = entry,
            allEntries = state.allEntries,
            useMiles = state.useMiles,
            route = state.route, // Matched watch GPS track, once available/consented (else null).
            onShowRoute = state.routeConsentId?.let { id -> { routeLauncher.launch(id) } },
            wearable = state.wearable, // That day's watch steps (null until loaded / when none).
            wearableConnected = state.stepsConnected, // Show an empty placeholder once connected.
            onEdit = viewModel::openEdit,
            onDelete = { confirmDelete = true },
            onBack = onBack
        )
    }

    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text("Delete entry?") },
            text = { Text("This can't be undone.") },
            confirmButton = {
                TextButton(onClick = { confirmDelete = false; viewModel.delete() }) { Text("Delete") }
            },
            dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text("Cancel") } }
        )
    }
}
