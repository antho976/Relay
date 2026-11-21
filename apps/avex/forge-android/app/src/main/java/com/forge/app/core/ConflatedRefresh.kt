package com.forge.app.core

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch

/** Collapse a burst, serialize reads, and retain one fresh request received during an active read. */
@OptIn(FlowPreview::class)
internal class ConflatedRefresh(scope: CoroutineScope, read: suspend () -> Unit) {
    private val requests = Channel<Unit>(Channel.CONFLATED)
    init { scope.launch { requests.receiveAsFlow().debounce(100).collect { read() } } }
    fun request() { requests.trySend(Unit) }
}
