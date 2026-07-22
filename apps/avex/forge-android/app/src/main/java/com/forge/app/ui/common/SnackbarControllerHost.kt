package com.forge.app.ui.common

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarData
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

// Lifts the snackbar clear of the 58dp [ForgeBottomBar] on the hub tabs, so it never sits over the
// nav tab targets. On deep routes (no bar) it just floats a little higher than the edge — fine.
private val SNACKBAR_BOTTOM = 66.dp

/**
 * Root-level host for the app's Undo snackbar ([SnackbarController]). Rendered once over the whole
 * app (a sibling of the [ProgramChangeGuardHost] in the app-root overlay), so an undo message shows
 * in place — including on the screen the caller popped back to after a delete.
 *
 * Newest-wins: a fresh event cancels any still-visible snackbar and shows itself, so rapid deletes
 * never queue up a backlog — the most recent action is always the one you can undo.
 */
@Composable
fun SnackbarControllerHost(
    viewModel: SnackbarControllerViewModel = hiltViewModel()
) {
    val hostState = remember { SnackbarHostState() }
    LaunchedEffect(viewModel) {
        var showing: Job? = null
        viewModel.controller.events.collect { event ->
            showing?.cancel() // dismiss the current snackbar so the newest replaces it immediately
            showing = launch {
                val result = hostState.showSnackbar(
                    message = event.message,
                    actionLabel = event.actionLabel,
                    withDismissAction = false,
                    duration = SnackbarDuration.Short,
                )
                if (result == SnackbarResult.ActionPerformed) {
                    event.onAction?.let { viewModel.runAction(it) }
                }
            }
        }
    }
    // The Box only hosts the bottom-anchored snackbar; its empty area draws nothing and holds no
    // pointer input, so taps pass straight through to the screen beneath.
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
        SnackbarHost(
            hostState = hostState,
            modifier = Modifier
                .navigationBarsPadding()
                .imePadding()
                .padding(horizontal = 12.dp)
                .padding(bottom = SNACKBAR_BOTTOM),
        ) { data -> ForgeSnackbar(data) }
    }
}

/** The app's snackbar, styled to the doctrine (§5): a dark [surface] plate, onBackground text, the
 *  accent on the Undo action — never Material's default light inverse-surface bar. */
@Composable
private fun ForgeSnackbar(data: SnackbarData) {
    Snackbar(
        snackbarData = data,
        containerColor = MaterialTheme.colorScheme.surface,
        contentColor = MaterialTheme.colorScheme.onBackground,
        actionColor = MaterialTheme.colorScheme.primary,
        shape = MaterialTheme.shapes.medium,
    )
}
