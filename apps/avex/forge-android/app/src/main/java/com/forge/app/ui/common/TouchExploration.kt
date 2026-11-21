package com.forge.app.ui.common

import android.content.Context
import android.view.accessibility.AccessibilityManager
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.platform.LocalContext

/**
 * Whether TalkBack touch-exploration is currently on. Read by [bounceClick] (and anything else that
 * needs to swap a screen-reader-friendly affordance in) so the value comes from ONE app-level observer
 * instead of every call site registering its own AccessibilityManager listener. Default false = the
 * sighted path; only meaningful inside [ProvideTouchExploration].
 */
val LocalTouchExplorationEnabled = staticCompositionLocalOf { false }

/**
 * Observes touch-exploration once for the whole composition and publishes it via
 * [LocalTouchExplorationEnabled]. Wrap the app content in this so toggling TalkBack mid-session
 * flips every dependent affordance live, with a single listener rather than O(elements).
 */
@Composable
fun ProvideTouchExploration(content: @Composable () -> Unit) {
    val context = LocalContext.current
    val a11yManager = remember(context) {
        context.getSystemService(Context.ACCESSIBILITY_SERVICE) as? AccessibilityManager
    }
    var enabled by remember { mutableStateOf(a11yManager?.isTouchExplorationEnabled == true) }
    DisposableEffect(a11yManager) {
        val mgr = a11yManager ?: return@DisposableEffect onDispose { }
        val listener = AccessibilityManager.TouchExplorationStateChangeListener { e -> enabled = e }
        mgr.addTouchExplorationStateChangeListener(listener)
        enabled = mgr.isTouchExplorationEnabled // resync in case it changed before we listened
        onDispose { mgr.removeTouchExplorationStateChangeListener(listener) }
    }
    CompositionLocalProvider(LocalTouchExplorationEnabled provides enabled, content = content)
}
