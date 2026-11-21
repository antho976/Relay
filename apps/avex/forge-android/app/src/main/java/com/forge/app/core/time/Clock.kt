package com.forge.app.core.time

import javax.inject.Inject
import javax.inject.Singleton

// The Clock fun-interface itself lives in :shared (com.forge.app.core.time.Clock) so the watch
// modules share it; only this Hilt-bound system implementation is Android-side.

@Singleton
class SystemClock @Inject constructor() : Clock {
    override fun nowMs(): Long = System.currentTimeMillis()
}

/**
 * `SystemClock.elapsedRealtime()` — milliseconds since boot, INCLUDING deep sleep. Monotonic: no
 * NTP correction or manual date change can move it, which is what makes it the right anchor for a
 * countdown (see [com.forge.app.core.time.ElapsedClock]).
 */
@Singleton
class SystemElapsedClock @Inject constructor() : ElapsedClock {
    override fun elapsedMs(): Long = android.os.SystemClock.elapsedRealtime()
}
