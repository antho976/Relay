package com.forge.wear.data

/**
 * The wrist's transient "you just logged a set" row: the set it names, when this watch heard about
 * it, and whether a rating has gone out for it. [rpeSent] hides the row — one rating per set.
 */
data class WristLastLog(val setId: Long, val atLocalMs: Long, val rpeSent: Boolean = false)

/**
 * One edit made on the wrist, replayable under its own id.
 *
 * The id matters as much as the payload: the phone dedupes by command id, so re-sending an edit
 * that may already have landed is only safe when it carries the SAME id — a fresh one reads as a
 * second, unrelated edit.
 */
data class WristEdit(
    val kind: Kind,
    val commandId: String,
    val sessionId: Long,
    val setId: Long?,
    val rpe: Double?
) {
    enum class Kind { RPE, UNDO }
}

/**
 * What an undelivered wrist edit does to the row it optimistically removed (M-10).
 *
 * Rating a set and undoing one both hid their own affordance before the transport was asked, and
 * then ignored what it said. Out of Bluetooth range the send never happened, the edit was never
 * queued, and the row that would have offered it again was already gone — the edit vanished, on the
 * one screen with nowhere to look it up.
 *
 * Removing the affordance optimistically is still right: the send almost always lands, and a wrist
 * row that hesitates for a second reads as broken. What was missing is the other half — putting it
 * back when the send definitively did not happen. Pure, so both halves are testable without a
 * watch, a phone, or a Bluetooth link between them.
 */
object WearEditRecovery {

    /**
     * The row after a rating for [ratedSetId] failed to leave the watch: the same row, offering to
     * rate again. A row that has since moved on to another set is left alone — that set's window is
     * over, and reopening it would invite a rating for the wrong set.
     */
    fun afterFailedRating(current: WristLastLog?, ratedSetId: Long): WristLastLog? =
        if (current?.setId == ratedSetId) current.copy(rpeSent = false) else current

    /**
     * The row after an undo failed, given what it [removed] when it was tapped. Restored only when
     * nothing has taken its place: a newer logged set means the user has moved on, and the older
     * row reappearing beneath it would offer to undo the wrong one.
     */
    fun afterFailedUndo(current: WristLastLog?, removed: WristLastLog?): WristLastLog? =
        current ?: removed
}
