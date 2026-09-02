package com.forge.app.ui.common

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Launch a small preference / grant mutation that must outlive the screen that started it.
 *
 * A preference is app state, not screen state, but a ViewModel's scope dies with its destination:
 * flip a Health Connect write-back toggle and swipe back immediately, and a bare
 * `viewModelScope.launch { repo.set(...) }` is cancelled mid-`DataStore.edit`, so the previous
 * value survives while the switch reported success (M-22). The write runs under [NonCancellable],
 * so the pop cancels nothing that was already in flight.
 *
 * Reserve it for the mutation itself, never a long job: a backup, import or backfill launched
 * this way could not be stopped, and the only reason to shield a write is that it finishes fast.
 * `viewModelScope` launches undispatched on `Main.immediate`, so the shield is entered before a
 * same-frame pop's cancellation can land.
 */
fun CoroutineScope.launchDurable(block: suspend () -> Unit): Job =
    launch { withContext(NonCancellable) { block() } }
