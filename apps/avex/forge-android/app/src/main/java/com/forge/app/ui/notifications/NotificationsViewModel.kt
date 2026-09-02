package com.forge.app.ui.notifications

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.AppNotice
import com.forge.app.data.repo.NotificationFeed
import com.forge.app.ui.common.SnackbarController
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import javax.inject.Inject

/**
 * Drives both the notifications page and the top-bar bell's unread count — one view model over the
 * shared [NotificationFeed], so the count in the chrome and the list on the page can never disagree.
 *
 * ONE instance, owned by the navigation host and handed to the notifications route. The route used
 * to ask Hilt for its own, scoped to the destination, so popping the page cancelled whatever that
 * copy was still writing (M-27). The mutations below are additionally shielded from cancellation:
 * a dismissal is app state, and Back is not a request to leave it half done.
 */
@HiltViewModel
class NotificationsViewModel @Inject constructor(
    private val feed: NotificationFeed,
    private val snackbar: SnackbarController,
) : ViewModel() {

    val notices: StateFlow<List<AppNotice>> =
        feed.notices.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    init { refresh() }

    /** Re-poll the coach pass + Health Connect grants (app open, and every resume). */
    fun refresh() = viewModelScope.launch { runCatching { feed.refresh() } }

    /** Opening the brief IS seeing it, so the row goes as the user navigates. */
    fun onCoachBriefOpened() = durable { feed.markCoachBriefSeen() }

    /**
     * Clear the list. Reversible, so it runs now and offers an Undo rather than asking first
     * (DESIGN §12) — the feed hands back the one operation that restores every row it cleared.
     *
     * The Undo is published only once EVERY dismissal has landed: the feed rolls back a partial
     * clear before failing, so a failure here means nothing was cleared and there is nothing to
     * offer. `runCatching` used to swallow the cancellation of a popped page as well, leaving rows
     * half cleared with no Undo at all (M-27).
     */
    fun clearAll() = viewModelScope.launch {
        val undo = try {
            withContext(NonCancellable) { feed.dismissAll() }
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            snackbar.show("Couldn't clear notifications. Try again.")
            return@launch
        }
        snackbar.showUndo("Notifications cleared") { undo() }
    }

    /**
     * A short persistence mutation that must outlive the screen it was tapped on. Shielded like
     * `SettingsViewModel.write`; cancellation is re-thrown rather than swallowed so a cancelled
     * coroutine still ends as one.
     */
    private fun durable(block: suspend () -> Unit) = viewModelScope.launch {
        try {
            withContext(NonCancellable) { block() }
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            // Best effort, as before: a failed seen-mark leaves the row for the next visit.
        }
    }
}
