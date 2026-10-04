package com.tally.app.ui.common

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.di.AppScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/** What the snackbar says when an Undo's restore fails (say its goal or account was deleted since). */
internal const val UNDO_FAILED = "That could not be undone"

/**
 * One message for the app's single snackbar. Compared by identity, so each post is its own notice
 * even when the words repeat. [undo] runs in the app scope, so leaving the screen cannot cancel it.
 */
class Notice(val message: String, val undo: (suspend () -> Unit)? = null)

/**
 * The app's ONE undo snackbar: undo over confirm. A reversible act happens at once and offers its
 * undo here; only erasing everything asks first. Rendered once at the root by [NoticeHost].
 *
 * One notice at a time, and the newest wins: a second delete replaces the first one's snackbar,
 * which commits the first delete, so the Undo on screen always reverses the act it names. The
 * notice lives here, in the app, not in the Activity: a rotation shows the same one again.
 */
@Singleton
class Notices @Inject constructor(@AppScope private val scope: CoroutineScope) {
    private val visible = MutableStateFlow<Notice?>(null)

    /** The notice on screen until it times out, is swiped away or is undone; null when none is. */
    internal val current: StateFlow<Notice?> = visible.asStateFlow()

    fun show(message: String) { visible.value = Notice(message) }

    fun showUndo(message: String, undo: suspend () -> Unit) { visible.value = Notice(message, undo) }

    /** The app's screen is gone for good (not a rotation): whatever was showing, its act stands. */
    internal fun clear() { visible.value = null }

    /**
     * [n] left the screen: its Undo was tapped ([undone]), or it timed out or was swiped away and its
     * act stands. Only the notice still current is acted on, so a newer one is never cleared by an
     * older one and an Undo runs at most once. The restore runs guarded: one that fails says so here
     * instead of taking the app down.
     */
    internal fun settle(n: Notice, undone: Boolean) {
        if (!visible.compareAndSet(n, null)) return
        val restore = n.undo
        if (!undone || restore == null) return
        scope.launch {
            try {
                restore()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                show(UNDO_FAILED)
            }
        }
    }
}

@Composable
fun NoticeHost(notices: Notices, state: SnackbarHostState, modifier: Modifier = Modifier) {
    val current by notices.current.collectAsStateWithLifecycle()
    // Keyed on the notice: a newer one cancels this wait, which takes the older snackbar (and its
    // Undo) off screen at once. A recreated Activity starts here again with the same notice.
    LaunchedEffect(current, state) {
        val n = current ?: return@LaunchedEffect
        val result = state.showSnackbar(
            message = n.message,
            actionLabel = if (n.undo != null) "Undo" else null,
            duration = SnackbarDuration.Short,
        )
        notices.settle(n, undone = result == SnackbarResult.ActionPerformed)
    }
    SnackbarHost(state, modifier) { data ->
        Snackbar(
            snackbarData = data,
            modifier = Modifier.padding(horizontal = 16.dp),
            containerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
            contentColor = MaterialTheme.colorScheme.onBackground,
            actionColor = MaterialTheme.colorScheme.primary,
        )
    }
}
