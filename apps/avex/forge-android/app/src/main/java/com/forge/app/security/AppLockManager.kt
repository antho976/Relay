package com.forge.app.security

import android.os.SystemClock
import com.forge.app.data.prefs.SettingsRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Process-scoped owner of the app / gallery lock state (GYMAP-69). It holds one in-memory
 * "session validity" bit: a successful unlock from EITHER gate validates the whole foreground
 * session, so unlocking the app also opens the gallery for free — no second prompt.
 *
 * Because this is a Hilt [Singleton] it lives for the process, not the Activity: [sessionValid]
 * survives Activity recreation (rotation) but starts `false` on a cold process start, which is
 * exactly the "lock on cold start, don't re-lock on rotation" behaviour we want. Re-locking after a
 * genuine backgrounding is governed by [SettingsRepository.appLockTimeoutSec] (0 = immediately).
 *
 * All state transitions are driven from the Activity's lifecycle callbacks on the main thread; the
 * settings collectors run on [Dispatchers.Main.immediate] too, so there are no cross-thread races.
 */
@Singleton
class AppLockManager @Inject constructor(
    settingsRepo: SettingsRepository,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    // Cached settings.
    @Volatile private var appLockEnabled = false
    @Volatile private var galleryLockEnabled = false
    @Volatile private var timeoutMs = 0L

    // Session state (process-scoped).
    @Volatile private var sessionValid = false
    @Volatile private var backgroundedAtElapsed = -1L

    private val _appLocked = MutableStateFlow(false)
    /** True while the whole-app gate should be shown over the nav host. */
    val appLocked: StateFlow<Boolean> = _appLocked.asStateFlow()

    private val _galleryLocked = MutableStateFlow(false)
    /** True while the photo-gallery gate should be shown. */
    val galleryLocked: StateFlow<Boolean> = _galleryLocked.asStateFlow()

    init {
        scope.launch { settingsRepo.appLockEnabled.collect { appLockEnabled = it; recompute() } }
        scope.launch { settingsRepo.galleryLockEnabled.collect { galleryLockEnabled = it; recompute() } }
        scope.launch { settingsRepo.appLockTimeoutSec.collect { timeoutMs = it * 1000L; recompute() } }
    }

    private fun recompute() {
        _appLocked.value = appLockEnabled && !sessionValid
        _galleryLocked.value = galleryLockEnabled && !sessionValid
    }

    /**
     * Seed both enabled flags synchronously on the first frame (before the async DataStore collectors
     * emit) so a locked cold start never flashes content underneath either gate. Idempotent and safe to
     * call on every Activity `onCreate`; it never touches [sessionValid], so it can't clear a valid
     * session on a rotation-triggered recreate.
     */
    fun primeEnabled(appEnabled: Boolean, galleryEnabled: Boolean) {
        appLockEnabled = appEnabled
        galleryLockEnabled = galleryEnabled
        recompute()
    }

    /** A successful unlock from either gate validates the whole foreground session. */
    fun markAuthenticated() {
        sessionValid = true
        backgroundedAtElapsed = -1L
        recompute()
    }

    /**
     * The app left the foreground — Home, Recents, the screen going off, a call, another app taking
     * over. Everything except a configuration change, which is not a backgrounding.
     *
     * This used to be gated on `MainActivity.userLeaving`, i.e. on
     * [android.app.Activity.onUserLeaveHint], which the framework raises only for a user's own
     * choice to leave. The power button and a display timeout raise none, so the re-lock timer never
     * started for the most common way a phone is put down. The icon-alias swap still needs that
     * distinction; the lock does not.
     */
    fun onGenuineBackground() {
        backgroundedAtElapsed = SystemClock.elapsedRealtime()
    }

    /** Back in the foreground — invalidate the session if the background grace elapsed. */
    fun onForeground() {
        val since = backgroundedAtElapsed
        backgroundedAtElapsed = -1L
        if (since >= 0 && SystemClock.elapsedRealtime() - since >= timeoutMs) {
            sessionValid = false
        }
        recompute()
    }
}
