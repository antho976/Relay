package com.tally.app.ui.common

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.tally.app.di.AppScope
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/** One message for the app's single snackbar. [undo] runs in the app scope, so leaving the screen cannot cancel it. */
data class Notice(val message: String, val undo: (suspend () -> Unit)? = null)

/**
 * The app's ONE undo snackbar: undo over confirm. A reversible act happens at once and offers its
 * undo here; only erasing everything asks first. Rendered once at the root by [NoticeHost].
 */
@Singleton
class Notices @Inject constructor(@AppScope private val scope: CoroutineScope) {
    private val channel = Channel<Notice>(Channel.BUFFERED)
    internal val flow = channel.receiveAsFlow()

    fun show(message: String) { channel.trySend(Notice(message)) }

    fun showUndo(message: String, undo: suspend () -> Unit) { channel.trySend(Notice(message, undo)) }

    internal fun runUndo(n: Notice) { n.undo?.let { scope.launch { it() } } }
}

@Composable
fun NoticeHost(notices: Notices, state: SnackbarHostState, modifier: Modifier = Modifier) {
    LaunchedEffect(notices, state) {
        notices.flow.collect { n ->
            val result = state.showSnackbar(
                message = n.message,
                actionLabel = if (n.undo != null) "Undo" else null,
                duration = SnackbarDuration.Short,
            )
            if (result == SnackbarResult.ActionPerformed) notices.runUndo(n)
        }
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
