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

/**
 * Monotonic elapsed time in milliseconds — Android's `SystemClock.elapsedRealtime()`.
 *
 * Deliberately separate from [Clock]: the two answer different questions and are wrong for each
 * other's job. [Clock] answers "what time is it", which is what a stored timestamp needs — and it
 * can jump, forward or backward, whenever NTP corrects a stale clock or the user sets the date.
 * This answers "how much time has passed", which is what a countdown needs, and it cannot jump.
 *
 * Unlike `System.nanoTime()` it keeps counting while the device is in deep sleep, so a rest timer
 * anchored on it still expires correctly with the screen off.
 */
fun interface ElapsedClock {
    fun elapsedMs(): Long
}
