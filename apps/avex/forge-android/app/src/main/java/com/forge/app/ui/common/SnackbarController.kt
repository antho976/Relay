package com.forge.app.ui.common

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The app's one Undo snackbar (§13 undo over confirm). A single injected instance any ViewModel can
 * reach, feeding a single [SnackbarControllerHost] hosted at the app root — so a "deleted · Undo"
 * message rides over whatever screen (or the screen the caller just popped back to) triggered it,
 * without every screen plumbing its own [androidx.compose.material3.SnackbarHostState].
 *
 * A destructive act runs immediately (the row vanishes) and offers a short window to reverse it; the
 * caller captures whatever it needs to restore and hands it back as [Event.onAction]. Mirrors the
 * shared-singleton + Hilt-bridge shape of [ProgramChangeGuard].
 */
@Singleton
class SnackbarController @Inject constructor() {
    /** One transient message; [onAction] runs if the user taps [actionLabel] before it dismisses. */
    data class Event(
        val message: String,
        val actionLabel: String? = null,
        val onAction: (suspend () -> Unit)? = null,
    )

    // BUFFERED so a rapid second delete never drops the first event on the floor; the host shows the
    // newest and cancels any still-visible predecessor, so at most one undo is live at a time.
    private val _events = Channel<Event>(Channel.BUFFERED)
    val events: Flow<Event> = _events.receiveAsFlow()

    /** A plain transient line, no action. */
    fun show(message: String) { _events.trySend(Event(message)) }

    /** A reversible action surfaced with an Undo affordance; [onUndo] restores what was removed. */
    fun showUndo(message: String, onUndo: suspend () -> Unit) {
        _events.trySend(Event(message, actionLabel = "Undo", onAction = onUndo))
    }
}

/** Bridges the singleton [SnackbarController] into Compose for the root host, and runs a tapped undo
 *  on a scope that survives the screen the delete was fired from being popped. */
@HiltViewModel
class SnackbarControllerViewModel @Inject constructor(
    val controller: SnackbarController
) : ViewModel() {
    fun runAction(action: suspend () -> Unit) = viewModelScope.launch { runCatching { action() } }
}
