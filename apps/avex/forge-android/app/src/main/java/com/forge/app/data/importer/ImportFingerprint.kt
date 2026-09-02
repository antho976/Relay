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
 * So the fingerprint is now every field the insert path actually writes and a user could change,
 * and nothing it derives (the nudged start instant, the denormalised volume, a set's completion
 * stamp when the source records none) — those differ between two copies of the same workout and
 * would report a re-import as new.
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
    val dropAnnotation: String?
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
