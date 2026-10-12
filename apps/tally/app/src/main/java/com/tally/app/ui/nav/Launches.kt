package com.tally.app.ui.nav

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.getAndUpdate
import javax.inject.Inject
import javax.inject.Singleton

/**
 * What the app was opened to do from outside it (a launcher shortcut: add an entry, search), held
 * from the moment the Activity receives it until the navigation can act on it.
 */
@Singleton
class Launches @Inject constructor() {
    private val held = MutableStateFlow<String?>(null)

    val pending: StateFlow<String?> = held.asStateFlow()

    fun offer(action: String) { held.value = action }

    fun take(): String? = held.getAndUpdate { null }

    companion object {
        const val ADD = "com.tally.app.action.ADD"
        const val SEARCH = "com.tally.app.action.SEARCH"
    }
}
