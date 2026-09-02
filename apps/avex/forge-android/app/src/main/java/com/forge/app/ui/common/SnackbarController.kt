package com.forge.app.ui.common

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.core.time.Clock
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.util.concurrent.atomic.AtomicLong
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
 *
 * The live event is STATE here, not a one-shot channel send. A channel event was consumed the moment
 * the host collected it, so an Activity recreation (rotation, resize) while the snackbar was up
 * disposed that host mid-`showSnackbar` and the next host had nothing to draw: the delete stayed
 * committed and its only Undo was gone. Now the event holds its own id and expiry, the host merely
 * REPLAYS whatever is current for the time it has left, and only an actual outcome — the action
 * taken ([take]) or the window running out ([dismiss]) — clears it. A host going away is not an
 * outcome. Newest still wins: a fresh event replaces the current one outright, and a clear names
 * the id it means so a late "timed out" from a replaced predecessor cannot take down its successor.
 */
@Singleton
class SnackbarController @Inject constructor(private val clock: Clock) {
    /** One transient message; [onAction] runs if the user taps [actionLabel] before [expiresAtMs]. */
    data class Event(
        /** Stable across host recreations, so a clear can name exactly the event it means. */
        val id: Long,
        val message: String,
        val actionLabel: String? = null,
        val onAction: (suspend () -> Unit)? = null,
        /** Wall-clock ms at which the window closes, however many hosts drew it in between. */
        val expiresAtMs: Long,
    )

    private val ids = AtomicLong()
    private val _current = MutableStateFlow<Event?>(null)

    /** The event on screen (or waiting for a host to draw it); null when nothing is live. */
    val current: StateFlow<Event?> = _current.asStateFlow()

    /** A plain transient line, no action. */
    fun show(message: String) = post(message, actionLabel = null, onAction = null)

    /** A reversible action surfaced with an Undo affordance; [onUndo] restores what was removed. */
    fun showUndo(message: String, onUndo: suspend () -> Unit) =
        post(message, actionLabel = "Undo", onAction = onUndo)

    private fun post(message: String, actionLabel: String?, onAction: (suspend () -> Unit)?) {
        // Newest wins: at most one undo is live, and it is always the most recent act.
        _current.value = Event(
            id = ids.incrementAndGet(),
            message = message,
            actionLabel = actionLabel,
            onAction = onAction,
            expiresAtMs = clock.nowMs() + WINDOW_MS
        )
    }

    /** How long [event] has left to be drawn, in ms; zero or less once its window has closed. */
    fun remainingMs(event: Event): Long = event.expiresAtMs - clock.nowMs()

    /** The window closed without its action (timed out, or swiped away). A no-op for any event that
     *  is no longer current, so a replaced predecessor can never clear its successor. */
    fun dismiss(id: Long) {
        _current.update { if (it?.id == id) null else it }
    }

    /** The user tapped the action: hands the event back to run, exactly once, and clears it. Null
     *  when [id] is no longer current — the tap landed on a snackbar a newer event had replaced. */
    fun take(id: Long): Event? {
        while (true) {
            val cur = _current.value
            if (cur?.id != id) return null
            if (_current.compareAndSet(cur, null)) return cur
        }
    }

    companion object {
        /** The undo window — Material's "short" snackbar, now measured on the clock rather than by
         *  whichever host happens to be composed. */
        const val WINDOW_MS = 4_000L
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
