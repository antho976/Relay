package com.forge.app.core.time

import javax.inject.Inject
import javax.inject.Singleton

// The Clock fun-interface itself lives in :shared (com.forge.app.core.time.Clock) so the watch
// modules share it; only this Hilt-bound system implementation is Android-side.

@Singleton
class SystemClock @Inject constructor() : Clock {
    override fun nowMs(): Long = System.currentTimeMillis()
}
