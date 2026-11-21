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

    // ── Durability across process death (M-10 / H-08) ─────────────────────────

    /**
     * A pending edit as one line of text, and back.
     *
     * The recovery above only ever lived in RAM: `_lastLog`, `_failedSend`, the command id and the
     * payload were all heap state on a singleton. Wear reclaims background processes aggressively,
     * and the window this exists for — the user out of range, waiting to walk back — is exactly the
     * window in which that happens. The edit and the only affordance for it disappeared together.
     *
     * Five fields, pipe-separated, behind a format version. The command id is a UUID and the rest
     * are numbers, so none of them can contain the separator; a line that does not parse is
     * discarded rather than guessed at, which loses no more than having no file at all.
     */
    fun encode(edit: WristEdit): String = listOf(
        FORMAT_V1,
        edit.kind.name,
        edit.commandId,
        edit.sessionId.toString(),
        edit.setId?.toString().orEmpty(),
        edit.rpe?.toString().orEmpty()
    ).joinToString(FIELD_SEPARATOR)

    /** @return the edit [line] describes, or null when it is absent, truncated or unrecognised. */
    fun decode(line: String?): WristEdit? {
        val parts = line?.trim()?.split(FIELD_SEPARATOR) ?: return null
        if (parts.size != 6 || parts[0] != FORMAT_V1) return null
        val kind = WristEdit.Kind.entries.firstOrNull { it.name == parts[1] } ?: return null
        val commandId = parts[2].takeIf { it.isNotEmpty() } ?: return null
        val sessionId = parts[3].toLongOrNull() ?: return null
        return WristEdit(
            kind = kind,
            commandId = commandId,
            sessionId = sessionId,
            setId = parts[4].takeIf { it.isNotEmpty() }?.toLongOrNull(),
            rpe = parts[5].takeIf { it.isNotEmpty() }?.toDoubleOrNull()
        )
    }

    private const val FORMAT_V1 = "v1"
    private const val FIELD_SEPARATOR = "|"
}
