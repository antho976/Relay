package com.forge.app.data.importer

/**
 * What makes one imported workout the SAME workout as one already in the log (M-03).
 *
 * The duplicate guard compares a workout in the file against the workouts already stored at the
 * same start instant, and skips it when they match. That test used to be reps, weight and hold
 * time only, so every other persisted field was invisible to it: re-importing a corrected export
 * whose only change was an assisted flag, an AMRAP or failure marker, a warm-up/set type, an RPE,
 * a difficulty tag, a note, an exercise's order or skipped flag, or the session's own
 * classification produced an identical fingerprint and the corrected copy was silently discarded —
 * on the one path whose promise is that it does not lose anything. An assisted pull-up staying
 * PR-eligible is the concrete cost.
 *
 * So the fingerprint is every field the insert path actually writes and a user could change, and
 * nothing it derives INDEPENDENTLY of the row it would be compared against — the nudged start
 * instant and the denormalised volume.
 *
 * The derived timings ARE covered, and covered by deriving them the same way on both sides (M-03).
 * A session's end time, its active duration and each set's completion stamp are the source's own
 * values when it states them and computed from the candidate start slot when it does not, so the
 * caller prints an incoming workout against each slot it could occupy rather than once. Printing
 * the raw source values instead would report every re-import from a source that omits them as new;
 * omitting them, which is what the first pass did, discarded an export corrected in one of them.
 *
 * The encoding is injective, not merely readable: free text is length-prefixed, so a note
 * containing the separator cannot forge a different workout's print. Pure, so the two sides of the
 * comparison — a parsed [ImportedSession] and a stored Session with its rows — are checkable
 * against each other in a JVM test rather than by inspection.
 */
internal data class FingerprintSet(
    val reps: Int,
    val weightLb: Double?,
    /** The stored `weight_text` — "2 plates" and "135" are the same load written two ways. */
    val weightText: String,
    val durationSeconds: Int?,
    val rpe: Double?,
    val isAssisted: Boolean,
    val isAmrap: Boolean,
    val toFailure: Boolean,
    val setType: String?,
    val difficultyTag: String?,
    val dropAnnotation: String?,
    /**
     * When this set was completed, as the row would STORE it — the source's own instant, or the
     * session's finish where the source records none (M-03).
     *
     * Derived on both sides against the same candidate start slot, which is what makes it safe to
     * compare: including the raw source value would report every re-import from a source that
     * omits it as new.
     */
    val completedAt: Long
)

internal data class FingerprintExercise(
    /** The stored `order_index`, which the source may state rather than leave to file position. */
    val orderIndex: Int,
    val exerciseId: String,
    val swappedName: String?,
    /** `EffortRating.name`, or null. */
    val difficulty: String?,
    val skipped: Boolean,
    val note: String?,
    val sets: List<FingerprintSet>
)

internal data class FingerprintSession(
    val dayKey: String,
    val sessionType: String,
    val intensity: String,
    val isUntracked: Boolean,
    val tags: String,
    val journal: String,
    /**
     * The three session-level values the first pass left out (M-03). They are parsed, persisted,
     * and a user can change them, so an export corrected in one of them alone was discarded as
     * identical: a fixed end time, a re-counted PR, or a mood added after the fact.
     *
     * [finishedAt] and [activeSeconds] are the values that would be STORED, derived against the
     * candidate start slot exactly as the insert derives them — see [FingerprintSet.completedAt].
     */
    val finishedAt: Long,
    val activeSeconds: Int,
    val prCount: Int,
    /** The `mood_entry` code written beside the session, or empty when the source records none. */
    val mood: String,
    val exercises: List<FingerprintExercise>
)

/**
 * The canonical print of [session]. Exercises are compared in stored order (`order_index`, ties
 * keeping their given order), which is the order the database reads them back in, so a file and
 * the rows written from it print the same however the parser listed them.
 */
internal fun fingerprintOf(session: FingerprintSession): String = buildString {
    append("s"); text(session.dayKey); text(session.sessionType); text(session.intensity)
    flag(session.isUntracked); text(session.tags); text(session.journal)
    append('t'); append(session.finishedAt); append('+'); append(session.activeSeconds)
    append('p'); append(session.prCount); text(session.mood)
    session.exercises.sortedBy { it.orderIndex }.forEach { ex ->
        append("|e"); append(ex.orderIndex); text(ex.exerciseId); text(ex.swappedName)
        text(ex.difficulty); flag(ex.skipped); text(ex.note)
        ex.sets.forEach { s ->
            append(";x"); append(s.reps)
            // A thousandth of a pound: a re-import carries bit-identical values, and no real source
            // distinguishes finer than that.
            append('@'); append(s.weightLb?.let { String.format(java.util.Locale.US, "%.3f", it) } ?: "bw")
            // A hold is not a rep count: a 60-second weighted plank and a 60-rep set at the same
            // load are different work, and one must never swallow the other.
            append('/'); append(s.durationSeconds?.toString() ?: "-")
            text(s.weightText)
            append('r'); append(s.rpe?.let { String.format(java.util.Locale.US, "%.2f", it) } ?: "-")
            flag(s.isAssisted); flag(s.isAmrap); flag(s.toFailure)
            text(s.setType); text(s.difficultyTag); text(s.dropAnnotation)
            append('c'); append(s.completedAt)
        }
    }
}

/** Length-prefixed so no value can contain a sequence that reads as the next field. */
private fun StringBuilder.text(value: String?) {
    if (value == null) append("~-") else { append('~'); append(value.length); append(':'); append(value) }
}

private fun StringBuilder.flag(value: Boolean) {
    append(if (value) '1' else '0')
}
