package com.forge.app.ui.notifications

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.AppNotice
import com.forge.app.data.repo.NotificationFeed
import com.forge.app.ui.common.SnackbarController
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Drives both the notifications page and the top-bar bell's unread count — one view model over the
 * shared [NotificationFeed], so the count in the chrome and the list on the page can never disagree.
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
    fun onCoachBriefOpened() = viewModelScope.launch { runCatching { feed.markCoachBriefSeen() } }

    /**
     * Clear the list. Reversible, so it runs now and offers an Undo rather than asking first
     * (DESIGN §12) — the feed hands back the one operation that restores every row it cleared.
     */
    fun clearAll() = viewModelScope.launch {
        val undo = runCatching { feed.dismissAll() }.getOrNull() ?: return@launch
        snackbar.showUndo("Notifications cleared") { undo() }
    }
}
