package com.forge.shared.protocol

/**
 * The wear Data Layer contract, v1 (docs/WEAR_OS_PLAN.md). One additive registry of path
 * constants: later revs (Engine E-C's /cardio/live, /cmd/cardio) ADD paths, never repurpose
 * existing ones, so a v1 watch keeps working against a v2 phone.
 *
 * DataItems carry STATE (latest-wins, survives disconnect); Messages carry COMMANDS (fire-once,
 * commandId-deduped, acked via the [WearProtocol.PATH_CMD_ACK] DataItem).
 */
object WearProtocol {

    /** Bumped only when an EXISTING payload changes shape incompatibly. Additive fields don't bump
     *  (the codec ignores unknown keys); a reader seeing a NEWER version than it knows shows
     *  "update the other app" and drops the payload — never a crash. */
    const val VERSION = 1

    // ── Phone → watch DataItems (state) ──────────────────────────────────────
    const val PATH_SESSION_LIVE = "/session/live"
    const val PATH_TIMER_STATE = "/timer/state"
    const val PATH_CONFIG = "/config"
    const val PATH_GLANCE_TODAY = "/glance/today"
    /** Command acknowledgements, keyed by commandId — the watch's pending→confirmed signal. */
    /**
     * Ack path PREFIX — each ack lands at `"$PATH_CMD_ACK/$commandId"`, never at the bare path.
     *
     * The Data Layer is a key-value store with latest-wins semantics per path and no guarantee that
     * intermediate values are delivered. One shared path therefore meant a second ack put while the
     * first was still unsynced simply superseded it: the watch matches acks by commandId, so the
     * first command's pending state never resolved, timed out to "Not logged · reconnecting" for a
     * set that HAD been logged, and invited the re-tap that duplicates it. A `needsConfirm` ack lost
     * the same way left the wrist unable to complete a legitimately heavy set at all.
     */
    const val PATH_CMD_ACK = "/cmd/ack"

    /** Where one command's ack lives. */
    fun ackPath(commandId: String): String = "$PATH_CMD_ACK/$commandId"

    // ── Watch → phone Messages (commands) ────────────────────────────────────
    const val PATH_CMD_LOG_SET = "/cmd/log-set"
    const val PATH_CMD_TIMER = "/cmd/timer"
    const val PATH_CMD_UNDO_SET = "/cmd/undo-set"
    /** Rate the set a log ack named ([CmdAckDto.setId]) — targeted, so a late RPE can't miss. */
    const val PATH_CMD_SET_RPE = "/cmd/set-rpe"
    const val PATH_HR_BATCH = "/hr/batch"
    /** The watch felt the timer-done buzz — lets the phone stay silent (one buzz, one body part). */
    const val PATH_HAPTIC_ACK = "/haptic/ack"

    /** The DataMap key every DataItem stores its serialized payload under. */
    const val KEY_PAYLOAD = "payload"

    /** Capability the wear app declares — the phone checks reachability against it. */
    const val CAPABILITY_WEAR_APP = "avex_wear_app"
}
