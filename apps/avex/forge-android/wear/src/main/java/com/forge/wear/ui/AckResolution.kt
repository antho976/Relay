package com.forge.wear.ui

import com.forge.shared.protocol.CmdAckDto

/** What [SessionScreen] should do with the ack that just arrived. */
internal sealed interface AckOutcome {
    /** Not the command this screen is waiting on — leave every piece of state alone. */
    data object Unrelated : AckOutcome
    data object Logged : AckOutcome
    data object NeedsConfirm : AckOutcome
    data class Refused(val reason: String) : AckOutcome
}

/**
 * Match [ack] against the command the screen is waiting on: the one in flight, or — after the
 * four-second timeout moved it aside — the one it timed out on.
 *
 * The timed-out id used to be matched only once the user re-tapped and it became pending again.
 * The phone now replays a recorded ack for a retried id (and a late first ack can arrive on its
 * own), and either can land while nothing is pending; ignoring it left the wrist reading
 * "Not logged · reconnecting" for a set that was in the database. Resolving on the timed-out id
 * clears that line — and drops the retry, since there is nothing left to resend.
 */
internal fun resolveAck(ack: CmdAckDto?, pendingId: String?, timedOutId: String?): AckOutcome {
    if (ack == null) return AckOutcome.Unrelated
    val awaited = pendingId ?: timedOutId ?: return AckOutcome.Unrelated
    if (ack.commandId != awaited) return AckOutcome.Unrelated
    return when {
        ack.ok -> AckOutcome.Logged
        ack.needsConfirm -> AckOutcome.NeedsConfirm
        else -> AckOutcome.Refused(ack.reason ?: "Not logged")
    }
}
