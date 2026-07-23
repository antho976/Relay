package com.forge.app.core.time

/**
 * Injectable wall-clock. Use this instead of System.currentTimeMillis() everywhere so
 * time-dependent logic (PR detection, weekly windows, streaks, the rest timer) can be
 * unit-tested with a FakeClock. Lives in :shared so the watch and phone share one clock
 * abstraction; the Hilt-bound SystemClock implementation stays in :app.
 */
fun interface Clock {
    fun nowMs(): Long
}
