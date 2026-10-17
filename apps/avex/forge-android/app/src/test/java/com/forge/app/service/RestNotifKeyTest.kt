package com.forge.app.service

import com.forge.app.domain.timer.RestTimerState
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The session notification re-posts on a rest's structural changes only. The system draws the
 * countdown itself, so a post per tick would be wasted work and a flicker on the lock screen.
 */
class RestNotifKeyTest {

    private fun key(remaining: Int, nowMs: Long, paused: Boolean = false) =
        RestNotifKey.of(RestTimerState(totalSeconds = 120, secondsRemaining = remaining, isPaused = paused), nowMs)

    @Test
    fun aTickingRestIsOneKey() {
        // 90 s left at t=0, 89 s left at t=1 s: the same end instant.
        assertTrue(key(90, 0).sameAs(key(89, 1_000)))
        assertTrue(key(90, 0).sameAs(key(30, 60_000)))
    }

    @Test
    fun addingThirtySecondsIsANewKey() {
        assertFalse(key(90, 0).sameAs(key(119, 1_000)))
    }

    @Test
    fun pausingResumingAndStoppingAreNewKeys() {
        assertFalse(key(90, 0).sameAs(key(90, 0, paused = true)))
        assertFalse(key(90, 0, paused = true).sameAs(key(90, 5_000)))
        assertFalse(key(90, 0).sameAs(RestNotifKey.of(null, 1_000)))
        assertFalse(key(1, 0).sameAs(key(0, 1_000)))
    }
}
