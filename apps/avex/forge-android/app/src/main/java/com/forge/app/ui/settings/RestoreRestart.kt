package com.forge.app.ui.settings

import android.content.Context
import android.content.Intent
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * A staged restore waiting for its restart.
 *
 * A restore only STAGES files; the next cold start swaps them in. The restart used to live in
 * Settings' own composition, so it only fired while Settings was on screen. Leave it once staging
 * had finished (Home, then back in through a widget that pops to Overview; or a widget pushing the
 * gym day on top) and the staged set sat waiting: whatever was logged next was wiped at the next
 * launch, maybe days later. This is process-wide and MainActivity restarts on it the moment it is
 * started, so nothing can be logged between staging and applying.
 */
object RestoreRestart {
    private val _pending = MutableStateFlow(false)
    val pending: StateFlow<Boolean> = _pending.asStateFlow()

    fun request() {
        _pending.value = true
    }

    /** Relaunch the app: the database file is swapped underneath Room at the next start. */
    fun relaunch(context: Context) {
        val intent = context.packageManager.getLaunchIntentForPackage(context.packageName)
        intent?.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
        if (intent != null) context.startActivity(intent)
        Runtime.getRuntime().exit(0)
    }
}
