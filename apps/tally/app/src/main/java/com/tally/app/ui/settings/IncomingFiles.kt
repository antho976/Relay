package com.tally.app.ui.settings

import android.net.Uri
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.getAndUpdate
import javax.inject.Inject
import javax.inject.Singleton

/**
 * A file shared into Tally (a statement downloaded in the browser, opened with Tally), held from
 * the moment the Activity receives it until the import screen takes it. One at a time: a second
 * share replaces a first one not yet read.
 */
@Singleton
class IncomingFiles @Inject constructor() {
    private val held = MutableStateFlow<Uri?>(null)

    /** The file waiting to be imported; null when none is. */
    val pending: StateFlow<Uri?> = held.asStateFlow()

    fun offer(uri: Uri) { held.value = uri }

    /** Hands the waiting file over, once. */
    fun take(): Uri? = held.getAndUpdate { null }
}
