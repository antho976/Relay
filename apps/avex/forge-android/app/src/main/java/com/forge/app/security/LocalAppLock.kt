package com.forge.app.security

import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.staticCompositionLocalOf

/**
 * The process's [AppLockManager], provided once at `MainActivity.setContent` so both the app-level
 * gate (MainActivity) and the gallery gate (ForgeNavHost) can read the lock state and mark a
 * successful unlock without threading the singleton through every composable. No default — a screen
 * that reads this must be under the provider.
 */
val LocalAppLock = staticCompositionLocalOf<AppLockManager> {
    error("LocalAppLock not provided — wrap content in CompositionLocalProvider(LocalAppLock provides ...)")
}

/**
 * True while the whole-app lock screen is up. Provided at the app root, and read by the lock-aware
 * window composables in `ui.common.window`: a dialog, sheet or popup is its own Android window and
 * would otherwise sit on top of the lock overlay. Defaults to false, so previews, tests and anything
 * composed outside the root need no provider.
 */
val LocalAppLockActive = compositionLocalOf { false }
