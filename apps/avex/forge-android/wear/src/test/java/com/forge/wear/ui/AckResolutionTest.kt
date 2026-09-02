package com.forge.wear.ui

import com.forge.shared.protocol.CmdAckDto
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The wrist's side of command acknowledgement (audit H-08): which ack the set screen accepts, and
 * in particular that one arriving AFTER the pending timeout — a late first ack, or the phone's
 * replay for a retried id — still resolves the "Not logged · reconnecting" state.
 */
class AckResolutionTest {

    private fun ack(
        id: String = "cmd-1",
        ok: Boolean = true,
        needsConfirm: Boolean = false,
        reason: String? = null
    ) = CmdAckDto(
        commandId = id, ok = ok, needsConfirm = needsConfirm, reason = reason,
        atMs = 1_000L, kind = CmdAckDto.KIND_LOG_SET
    )

    @Test
    fun theAckForThePendingCommandResolvesIt() {
        assertEquals(AckOutcome.Logged, resolveAck(ack(), pendingId = "cmd-1", timedOutId = null))
    }

    @Test
    fun anAckArrivingAfterTheTimeoutResolvesTheTimedOutCommand() {
        // Nothing is pending any more — the screen reads "Not logged · reconnecting" — but the
        // phone's replayed ack for that same id says the set landed.
        assertEquals(AckOutcome.Logged, resolveAck(ack(), pendingId = null, timedOutId = "cmd-1"))
    }

    @Test
    fun anAckForSomeOtherCommandIsIgnored() {
        assertEquals(AckOutcome.Unrelated, resolveAck(ack("cmd-9"), pendingId = "cmd-1", timedOutId = null))
        assertEquals(AckOutcome.Unrelated, resolveAck(ack("cmd-9"), pendingId = null, timedOutId = "cmd-1"))
    }

    @Test
    fun nothingAwaitedMeansNothingResolves() {
        assertEquals(AckOutcome.Unrelated, resolveAck(ack(), pendingId = null, timedOutId = null))
        assertEquals(AckOutcome.Unrelated, resolveAck(null, pendingId = "cmd-1", timedOutId = null))
    }

    @Test
    fun thePendingCommandTakesPrecedenceOverAStaleTimedOutOne() {
        // A new set is in flight; an ack for the older, timed-out id must not resolve the new one.
        assertEquals(
            AckOutcome.Unrelated,
            resolveAck(ack("cmd-old"), pendingId = "cmd-new", timedOutId = "cmd-old")
        )
    }

    @Test
    fun aRefusalCarriesThePhonesReasonOrTheQuietDefault() {
        assertEquals(
            AckOutcome.Refused("stale session"),
            resolveAck(ack(ok = false, reason = "stale session"), pendingId = "cmd-1", timedOutId = null)
        )
        assertEquals(
            AckOutcome.Refused("Not logged"),
            resolveAck(ack(ok = false), pendingId = "cmd-1", timedOutId = null)
        )
    }

    @Test
    fun aBigJumpAsksForConfirmation() {
        assertEquals(
            AckOutcome.NeedsConfirm,
            resolveAck(ack(ok = false, needsConfirm = true), pendingId = "cmd-1", timedOutId = null)
        )
    }
}
