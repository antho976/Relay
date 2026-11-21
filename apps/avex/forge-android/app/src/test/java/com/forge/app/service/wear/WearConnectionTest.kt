package com.forge.app.service.wear

import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The timer-haptic handoff ledger (DESIGN.md §16: one buzz, on the body part that feels it).
 *
 * The rule the phone is enforcing is "the wrist already buzzed for THIS timer, so stay silent".
 * It used to test only "the wrist buzzed recently", over an eight-second window sized for
 * Bluetooth lag — which is long enough to span the end of one rest and the start of the next.
 * Skip a finished rest, start the next set, and the new timer's phone-side buzz was suppressed by
 * the previous timer's ack, while the wrist would not buzz again for a timer it had already
 * acked. Neither device buzzed, which is the one outcome the whole handoff exists to prevent.
 */
@RunWith(RobolectricTestRunner::class)
class WearConnectionTest {

    private val window = 8_000L
    private fun connection() = WearConnection(ApplicationProvider.getApplicationContext())

    @Test
    fun `an ack for this timer silences the phone`() {
        val c = connection()
        c.recordHapticAck(timerEndAtMs = 1_000L, atMs = 100L)
        assertTrue(c.hapticAckedFor(1_000L, window, nowMs = 2_000L))
    }

    @Test
    fun `an ack for the previous timer does not silence the next one`() {
        val c = connection()
        // The rest that just ended, acked by the wrist.
        c.recordHapticAck(timerEndAtMs = 1_000L, atMs = 100L)
        // A new rest, started two seconds later — well inside the recency window.
        assertFalse(c.hapticAckedFor(9_000L, window, nowMs = 2_100L))
    }

    @Test
    fun `an ack that is too old does not silence a re-run of the same timer`() {
        val c = connection()
        c.recordHapticAck(timerEndAtMs = 1_000L, atMs = 100L)
        assertFalse(c.hapticAckedFor(1_000L, window, nowMs = 100L + window + 1))
    }

    @Test
    fun `no ack at all means the phone buzzes`() {
        assertFalse(connection().hapticAckedFor(1_000L, window, nowMs = 5_000L))
    }

    @Test
    fun `an unidentified timer never matches`() {
        val c = connection()
        // A watch build predating HapticAckDto.timerEndAtMs, or a phone with no running timer
        // published. Zero is "no identity", and the safe direction is a late phone buzz.
        c.recordHapticAck(timerEndAtMs = 0L, atMs = 100L)
        assertFalse(c.hapticAckedFor(0L, window, nowMs = 200L))
    }

    @Test
    fun `a later ack replaces the timer it is remembered against`() {
        val c = connection()
        c.recordHapticAck(timerEndAtMs = 1_000L, atMs = 100L)
        c.recordHapticAck(timerEndAtMs = 9_000L, atMs = 200L)
        assertTrue(c.hapticAckedFor(9_000L, window, nowMs = 300L))
        assertFalse(c.hapticAckedFor(1_000L, window, nowMs = 300L))
    }

    @Test
    fun `an out-of-order ack does not overwrite a newer one`() {
        val c = connection()
        c.recordHapticAck(timerEndAtMs = 9_000L, atMs = 200L)
        c.recordHapticAck(timerEndAtMs = 1_000L, atMs = 100L)
        assertTrue(c.hapticAckedFor(9_000L, window, nowMs = 300L))
    }
}
