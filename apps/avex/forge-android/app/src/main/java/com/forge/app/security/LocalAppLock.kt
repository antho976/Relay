package com.forge.app.security

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
