package com.forge.app.domain.program

import org.json.JSONObject

/**
 * The record a program regeneration leaves behind while it is in flight, so the deload state and
 * the program it describes cannot end up permanently disagreeing (M-06).
 *
 * Two facts live in two stores. The plan is Room rows; "you are in a deload week" is a DataStore
 * timestamp. Nothing can write both atomically, so the order used to be: mark the deload, then
 * regenerate. A generate that threw left the marker set over the old full-volume plan, forever —
 * not a race, just a failure. Reversing the order fixes that half and opens the mirror one: a
 * normal regenerate commits its rows and the process dies before the marker clears.
 *
 * So the intent is written first and completed after. It carries [beforeSignature] — the signature
 * of the program as it stood BEFORE the transaction — which is what lets a later boot tell the two
 * apart without guessing: if the program on disk still matches it, the transaction never committed
 * and the intent is discarded; if it does not, the rows landed and the marker is applied now.
 * A deload always changes the plan's set counts, so the two cases never look alike.
 */
data class ProgramGenerationIntent(
    /** True when the regeneration being attempted is a deload. */
    val deload: Boolean,
    /** The instant the attempt began — the deload week is anchored on it, not on the boot that finishes it. */
    val atMs: Long,
    /** [programSignature] of the program this attempt is replacing. */
    val beforeSignature: String,
    /**
     * [programSignature] of the program this attempt is about to WRITE (M-06).
     *
     * `beforeSignature` alone can only say "something changed", and something else changing is not
     * this attempt committing. A generation that failed, followed by a custom save or a reroll
     * before the next boot, left a program matching neither — and the boot applied the abandoned
     * attempt's deload marker to a plan it had never touched. Recognising the attempt's OWN result
     * is what tells a commit from someone else's write.
     *
     * Empty for an intent written by a build that predates it; that falls back to the old rule.
     */
    val afterSignature: String = "",
    /** Identifies the attempt, so an intent found later can be told from a fresh one. */
    val opId: String = ""
) {
    fun toJson(): String = JSONObject().apply {
        put("deload", deload)
        put("atMs", atMs)
        put("before", beforeSignature)
        put("after", afterSignature)
        put("op", opId)
    }.toString()

    companion object {
        /** Tolerant parse: null / blank / corrupt all read as "no attempt in flight". */
        fun fromJson(json: String?): ProgramGenerationIntent? {
            if (json.isNullOrBlank()) return null
            return runCatching {
                val o = JSONObject(json)
                ProgramGenerationIntent(
                    deload = o.optBoolean("deload", false),
                    atMs = o.optLong("atMs", 0L).takeIf { it > 0L } ?: return null,
                    beforeSignature = o.optString("before"),
                    afterSignature = o.optString("after"),
                    opId = o.optString("op")
                )
            }.getOrNull()
        }
    }
}

/** What a boot should do with a pending intent. See [resolvePendingGeneration]. */
sealed interface PendingGeneration {
    /** Nothing was in flight. */
    data object None : PendingGeneration

    /** The transaction never committed: forget the attempt and leave the deload marker alone. */
    data object Discard : PendingGeneration

    /**
     * The rows landed but the marker write did not. Apply [deloadStartMs] now: the attempt's own
     * instant for a deload, and 0 — "not in a deload week" — for an ordinary regeneration.
     */
    data class Apply(val deloadStartMs: Long) : PendingGeneration

    /**
     * The program on disk is neither what the attempt was replacing nor what it would have written:
     * something else has changed it since. The attempt's marker must NOT be applied to a plan it
     * never produced — the intent is dropped and the deload state left as it is (M-06).
     */
    data object Superseded : PendingGeneration
}

/**
 * Whether the [intent] found at boot got as far as writing its program, judged by comparing the
 * signature it recorded against [currentSignature], the program actually on disk now.
 */
fun resolvePendingGeneration(
    intent: ProgramGenerationIntent?,
    currentSignature: String
): PendingGeneration {
    if (intent == null) return PendingGeneration.None
    val marker = PendingGeneration.Apply(if (intent.deload) intent.atMs else 0L)
    return when {
        // The attempt's OWN result is on disk: it committed. Checked FIRST, so a regeneration whose
        // output happens to equal its input — deterministic picks, constrained equipment — is read
        // as the commit it was rather than as "nothing changed". Under the old rule that case was
        // indistinguishable from a failure, and a marker the user had asked for was discarded.
        intent.afterSignature.isNotEmpty() && currentSignature == intent.afterSignature -> marker

        // Untouched: the transaction never committed. The marker is left exactly as it was.
        currentSignature == intent.beforeSignature -> PendingGeneration.Discard

        // An intent from a build that recorded no `after` can only use the old rule: anything that
        // is not the before-program reads as a commit.
        intent.afterSignature.isEmpty() -> marker

        // Neither. A custom save, a reroll, or a later generation has written since, and this
        // attempt's marker describes none of them.
        else -> PendingGeneration.Superseded
    }
}

/**
 * A stable content signature for a program: its days in order, each with its slots' movements, set
 * counts and rep ranges. Deliberately covers exactly what a regeneration rewrites, so a committed
 * transaction is always visible as a change and nothing else is.
 *
 * Taken as flat strings rather than the Room entities so the whole reconciliation is testable on
 * the JVM without a database.
 */
fun programSignature(days: List<ProgramSignatureDay>): String =
    days.joinToString("|") { d ->
        "${d.id}@${d.archetype}:" + d.slots.joinToString(",") { s -> "${s.exerciseLibId}x${s.sets}/${s.reps}" }
    }

data class ProgramSignatureSlot(val exerciseLibId: String, val sets: Int, val reps: String)
data class ProgramSignatureDay(val id: String, val archetype: String, val slots: List<ProgramSignatureSlot>)
