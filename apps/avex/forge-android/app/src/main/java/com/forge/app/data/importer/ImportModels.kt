package com.forge.app.data.importer

/**
 * Normalized intermediate model for importing training history from OTHER gym apps (#GYMAP-17).
 *
 * Every supported source (Strong, Hevy, FitNotes, a generic CSV, or Forge's own JSON export) is
 * parsed by its [GymImporter] into this app-neutral shape, then a single insert path
 * ([WorkoutImportRepository]) maps exercise names to the code catalogue and writes real
 * Session/LoggedExercise/LoggedSet rows. Keeping the parsers behind this model means the insert
 * logic — name matching, unit handling, volume denormalisation — lives in exactly one place and
 * every app benefits from it.
 *
 * All weights here are ALREADY in pounds — each importer converts from the source's unit (kg rows,
 * a per-row unit column) up front, so nothing downstream has to think about units.
 */

/** One performed set. [weightLb] is null for a bodyweight / non-numeric entry (contributes 0 volume). */
data class ImportedSet(
    val weightLb: Double?,
    val reps: Int,
    val rpe: Double? = null,
    /** True when the source marked this as a warm-up set — kept so we could filter later; imported as normal today. */
    val isWarmup: Boolean = false
)

/**
 * One exercise within a workout. Normally identified only by its source [name] (matched to the
 * catalogue on insert). [catalogueId] lets a source that ALREADY knows the real catalogue id — the
 * Avex JSON export carries it — pin the link directly instead of round-tripping through name
 * matching, which would silently de-link movements whose display name doesn't re-match their own id.
 */
data class ImportedExercise(
    val name: String,
    val sets: List<ImportedSet>,
    val note: String? = null,
    val catalogueId: String? = null
)

/**
 * One workout. [startedAtMs] is the source's date/time (midnight when only a date is given).
 * [finishedAtMs] is null when the source records no end time — the importer stamps a nominal
 * duration so the session still reads as finished.
 */
data class ImportedSession(
    val startedAtMs: Long,
    val finishedAtMs: Long?,
    val exercises: List<ImportedExercise>,
    val title: String? = null,
    val note: String? = null
)

/** Which app a file was recognised as — drives the confirmation copy and the result summary. */
enum class ImportSource(val displayName: String) {
    STRONG("Strong"),
    HEVY("Hevy"),
    FITNOTES("FitNotes"),
    FORGE_JSON("Avex export"),
    GENERIC_CSV("CSV file")
}

/**
 * Outcome of an import. [sessions]/[exercises]/[sets] count what was written; [matchedExercises]
 * is how many distinct exercise names resolved to a known catalogue movement (the rest are kept
 * under their original name). [skippedRows] counts source rows that couldn't be parsed into a set.
 */
sealed interface ImportResult {
    data class Success(
        val source: ImportSource,
        val sessions: Int,
        val exercises: Int,
        val sets: Int,
        val matchedExercises: Int,
        val unmatchedExercises: Int,
        val skippedRows: Int,
        /** Workouts already present (same start time) that were skipped to avoid double-importing. */
        val duplicatesSkipped: Int = 0
    ) : ImportResult

    /** File was read but nothing usable was found in it (empty, or no rows we could parse). */
    data object NothingToImport : ImportResult

    /** File format wasn't recognised as any supported gym app export. */
    data object UnrecognisedFormat : ImportResult

    /** File was too large to be a plausible workout export (guards against picking the wrong file). */
    data object TooLarge : ImportResult

    /** Could not read the chosen file at all. */
    data object ReadError : ImportResult
}

/** Plain-English outcome for the user — one place so every import entry point (Settings pick, folder
 *  scan, share-to-Avex) words the result identically. Dry, no exclamation marks (DESIGN §11). */
fun ImportResult.userMessage(): String = when (this) {
    is ImportResult.Success -> buildString {
        append("Imported $sessions ")
        append(if (sessions == 1) "workout" else "workouts")
        append(" from ${source.displayName} · $sets sets.")
        if (duplicatesSkipped > 0) {
            append(" $duplicatesSkipped already in your log ")
            append(if (duplicatesSkipped == 1) "was" else "were")
            append(" skipped.")
        }
        if (unmatchedExercises > 0) {
            append(" $unmatchedExercises ")
            append(if (unmatchedExercises == 1) "exercise wasn't" else "exercises weren't")
            append(" in the library — kept under their original names.")
        }
    }
    ImportResult.NothingToImport -> "No new workouts found in that file."
    ImportResult.UnrecognisedFormat ->
        "That file isn't a recognised gym-app export. Export a CSV from Strong, Hevy, FitNotes, or a similar app, or an Avex JSON export."
    ImportResult.TooLarge -> "That file is too large to import."
    ImportResult.ReadError -> "Couldn't read that file. Try again, or pick a different copy."
}
