package com.forge.app.service.wear

import com.forge.app.service.wear.WearStatePublisher.Companion.hapticIdentityAfter
import com.forge.shared.protocol.TimerStateDto
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The identity the phone matches the wrist's haptic ack against (2026-09-26 audit, 11).
 *
 * A rest that runs out is republished as paused-at-zero, and that publish reset the identity to 0
 * before [com.forge.app.service.WorkoutSessionService] read it. `hapticAckedFor(0, …)` never
 * matches, so the phone buzzed and posted a heads-up on top of the wrist at every rest.
 */
class WearHapticIdentityTest {

    private fun running(endAtMs: Long) = TimerStateDto(endAtMs = endAtMs, totalSeconds = 90)
    private fun paused(remaining: Int) =
        TimerStateDto(endAtMs = 0L, totalSeconds = 90, paused = true, pausedRemainingSeconds = remaining)

    @Test
    fun `a running rest is identified by its end instant`() {
        assertEquals(10_000L, hapticIdentityAfter(previous = 0L, dto = running(10_000L)))
    }

    @Test
    fun `the expiry publish keeps the identity the wrist will ack with`() {
        val atStart = hapticIdentityAfter(previous = 0L, dto = running(10_000L))
        assertEquals(10_000L, hapticIdentityAfter(previous = atStart, dto = paused(remaining = 0)))
    }

    @Test
    fun `a stop clears the identity`() {
        assertEquals(0L, hapticIdentityAfter(previous = 10_000L, dto = null))
    }

    @Test
    fun `a manual pause clears the identity`() {
        // A paused timer cannot expire, and resuming publishes a fresh end instant.
        assertEquals(0L, hapticIdentityAfter(previous = 10_000L, dto = paused(remaining = 42)))
    }

    @Test
    fun `the next rest replaces the expired one`() {
        val expired = hapticIdentityAfter(previous = 10_000L, dto = paused(remaining = 0))
        assertEquals(95_000L, hapticIdentityAfter(previous = expired, dto = running(95_000L)))
    }
}
