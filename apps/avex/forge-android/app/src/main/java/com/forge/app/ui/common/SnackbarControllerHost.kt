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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.withTimeoutOrNull

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
 *
 * The host only REPLAYS the controller's current event for whatever time it has left; it never owns
 * it. A host disposed mid-snackbar (Activity recreation on rotation) therefore acknowledges nothing,
 * and the host composed next draws the same event again with the same Undo, until the user takes it
 * or its window closes on the clock.
 */
@Composable
fun SnackbarControllerHost(
    viewModel: SnackbarControllerViewModel = hiltViewModel()
) {
    val hostState = remember { SnackbarHostState() }
    val controller = viewModel.controller
    val event by controller.current.collectAsStateWithLifecycle()
    LaunchedEffect(event) {
        val current = event ?: return@LaunchedEffect
        val remaining = controller.remainingMs(current)
        if (remaining <= 0L) {
            controller.dismiss(current.id)
            return@LaunchedEffect
        }
        // Indefinite + our own timeout: the window is measured from when the event was posted, so a
        // replay after recreation shows only what is left of it rather than a fresh four seconds.
        // Cancellation (a newer event, or this host leaving composition) drops out of here before
        // the `when`, which is exactly the "not an outcome" case the controller is built around.
        val result = withTimeoutOrNull(remaining) {
            hostState.showSnackbar(
                message = current.message,
                actionLabel = current.actionLabel,
                withDismissAction = false,
                duration = SnackbarDuration.Indefinite,
            )
        }
        when (result) {
            SnackbarResult.ActionPerformed ->
                controller.take(current.id)?.onAction?.let { viewModel.runAction(it) }
            // Timed out (null) or swiped away: over, without its action.
            else -> controller.dismiss(current.id)
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
