package com.forge.app.core.time

import javax.inject.Inject
import javax.inject.Singleton

/**
 * Injectable wall-clock. Use this instead of System.currentTimeMillis() everywhere so
 * time-dependent logic (PR detection, weekly windows, streaks) can be unit-tested with
 * a FakeClock.
 */
fun interface Clock {
    fun nowMs(): Long
}

@Singleton
class SystemClock @Inject constructor() : Clock {
    override fun nowMs(): Long = System.currentTimeMillis()
}
