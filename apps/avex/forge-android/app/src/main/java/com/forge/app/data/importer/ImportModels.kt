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

/**
 * One performed set. [weightLb] is null for a bodyweight / non-numeric entry (contributes 0 volume).
 *
 * [durationSeconds] and [isAssisted] are not decoration: a set with a duration is a timed HOLD, so
 * its `reps` is not a count and it must stay out of every weight x reps aggregate, and an assisted
 * set is excluded from all-time PR comparison (see `LoggedSet`). Dropping either on the way through
 * this model turned a 90-second weighted plank into a 90-rep 45 lb set and made band-assisted
 * pull-ups PR-eligible — on the migration path the Avex JSON export exists to serve.
 */
data class ImportedSet(
    val weightLb: Double?,
    val reps: Int,
    /**
     * What the user typed, verbatim ("BW", "2 plates", "45") — already canonical lb text, which is
     * what `LoggedSet.weightText` stores and what the UI shows back. Null for a source that doesn't
     * record it (every foreign CSV); the insert path then regenerates a bare lb number as before.
     *
     * Avex's own JSON export writes this field, and it used to be dropped on the way back in: a set
     * logged as "2 plates" round-tripped as "135", so re-importing your own history silently
     * rewrote your notation across every set of it.
     */
    val weightText: String? = null,
    val rpe: Double? = null,
    /** True when the source marked this as a warm-up set. Imported as a "warmup" set type. */
    val isWarmup: Boolean = false,
    /** Timed hold: `reps` is not a count and the set is excluded from weight x reps aggregates. */
    val durationSeconds: Int? = null,
    /** Bands / spotter — excluded from all-time PR comparison. */
    val isAssisted: Boolean = false,
    /** Intent was max reps; `reps` is what was achieved. */
    val isAmrap: Boolean = false,
    /** Set ended at muscular failure. */
    val toFailure: Boolean = false,
    /** Advanced set type: null = normal | "warmup" | "drop" | "myo" | "rest_pause". */
    val setType: String? = null,
    /** Per-set effort tag, when the source records one. */
    val difficultyTag: String? = null,
    /** Drop-set annotation ("60 → 40"), when the source records one. */
    val dropAnnotation: String? = null,
    /** When the set was completed. Null for a source that only times the workout, not each set —
     *  the insert path then stamps the session's finish instant, as it always did. */
    val completedAtMs: Long? = null
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
    val catalogueId: String? = null,
    /** Source-recorded position within the workout. Null → the array position is used. */
    val orderIndex: Int? = null,
    /** `EffortRating` name as the source spelled it; unrecognised values are ignored on insert. */
    val difficulty: String? = null,
    /** The user marked this exercise skipped. Skipped exercises are excluded from the engine's
     *  population, so importing one as "performed" silently rewrites the training history. */
    val skipped: Boolean = false
)

/**
 * One workout. [startedAtMs] is the source's date/time (midnight when only a date is given).
 * [finishedAtMs] is null when the source records no end time — the importer stamps a nominal
 * duration so the session still reads as finished.
 *
 * Everything below [note] is only ever populated by Avex's own JSON export, which is the one source
 * that knows these things. Other apps leave them null and the insert path falls back to exactly the
 * defaults it used before.
 */
data class ImportedSession(
    val startedAtMs: Long,
    val finishedAtMs: Long?,
    val exercises: List<ImportedExercise>,
    val title: String? = null,
    val note: String? = null,
    /**
     * Real ACTIVE training seconds, which is NOT `finishedAt - startedAt`: a "resume later" session
     * can span two days while holding 70 minutes of training. When a source records this, it wins
     * over any wall-clock derivation.
     */
    val activeSeconds: Int? = null,
    /** The program day this workout belongs to. Null → imported as an open/freestyle session. */
    val dayKey: String? = null,
    /** "normal" | "deload" | "test" | "technique" | "first_back". */
    val sessionType: String? = null,
    /** "light" | "normal" | "hard". */
    val intensity: String? = null,
    /** Excluded from streak, trophies and suggestions — a fact about the workout, not a preference. */
    val isUntracked: Boolean = false,
    /** Comma-separated quick tags. */
    val tags: String? = null,
    /** Post-session mood code, written to `mood_entry` alongside the session. */
    val mood: String? = null,
    /** PRs the source recorded for this workout. Null → recomputed as 0, as before. */
    val prCount: Int? = null
)

/**
 * The non-workout rows an Avex JSON export also carries. They were written by every export and read
 * by nothing, so a device-to-device migration lost every cardio entry and every coach goal without
 * saying so. [GymImporter.parseExtras] defaults to empty, so no other app's parser has to care.
 */
data class ImportedExtras(
    val cardio: List<ImportedCardio> = emptyList(),
    val coachGoals: List<ImportedCoachGoal> = emptyList(),
    val bodyweight: List<ImportedBodyweight> = emptyList()
) {
    val isEmpty: Boolean get() = cardio.isEmpty() && coachGoals.isEmpty() && bodyweight.isEmpty()
}

/** One weigh-in. [dateKey] is the `yyyy-MM-dd` the entry is filed under; weight is canonical lb. */
data class ImportedBodyweight(val dateKey: String, val weightLb: Double)

/** One cardio / rest-day entry. [dateMs] is the entry's own date column, verbatim. */
data class ImportedCardio(
    val dateMs: Long,
    val type: String,
    val durationMin: Int,
    val distanceKm: Double? = null,
    val effort: String? = null,
    val restReason: String? = null,
    val note: String? = null,
    val inclinePct: Double? = null,
    val laps: Int? = null,
    val elevationM: Double? = null
)

/** One coach goal. */
data class ImportedCoachGoal(
    val kind: String,
    val targetKey: String,
    val targetValue: Double? = null,
    val priority: Int = 0,
    val createdAt: Long,
    val completedAt: Long? = null,
    val archivedAt: Long? = null,
    val source: String = "user",
    val note: String = ""
)

/** Which app a file was recognised as — drives the confirmation copy and the result summary. */
enum class ImportSource(val displayName: String) {
    STRONG("Strong"),
    HEVY("Hevy"),
    FITNOTES("FitNotes"),
    FORGE_JSON("Avex export"),
    FORGE_BODYWEIGHT_CSV("Avex bodyweight export"),
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
        val duplicatesSkipped: Int = 0,
        /** Cardio entries written (Avex JSON export only). */
        val cardioEntries: Int = 0,
        /** Coach goals written (Avex JSON export only). */
        val coachGoals: Int = 0,
        /** Weigh-ins written (Avex bodyweight CSV). */
        val bodyweightEntries: Int = 0
    ) : ImportResult

    /** The file is an Avex export written by a NEWER format version than this build understands.
     *  Reading it with the current parser would silently mis-import it, so we refuse instead. */
    data class UnsupportedExportVersion(val version: Int) : ImportResult

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
        if (sessions == 0 && sets == 0) {
            // A bodyweight or cardio-only file has no workouts to count; leading with "Imported 0
            // workouts · 0 sets" would read as a failure.
            setLength(0)
            append("Imported from ${source.displayName}.")
        } else {
            append(" from ${source.displayName} · $sets sets.")
        }
        if (duplicatesSkipped > 0) {
            append(" $duplicatesSkipped already in your log ")
            append(if (duplicatesSkipped == 1) "was" else "were")
            append(" skipped.")
        }
        if (cardioEntries > 0) {
            append(" $cardioEntries cardio ")
            append(if (cardioEntries == 1) "entry" else "entries")
            append(".")
        }
        if (coachGoals > 0) {
            append(" $coachGoals coach ")
            append(if (coachGoals == 1) "goal" else "goals")
            append(".")
        }
        if (bodyweightEntries > 0) {
            append(" $bodyweightEntries ")
            append(if (bodyweightEntries == 1) "weigh-in" else "weigh-ins")
            append(".")
        }
        if (unmatchedExercises > 0) {
            append(" $unmatchedExercises ")
            append(if (unmatchedExercises == 1) "exercise wasn't" else "exercises weren't")
            append(" in the library — kept under their original names.")
        }
    }
    is ImportResult.UnsupportedExportVersion ->
        "That Avex export was written by a newer version of the app (format $version). Update Avex, then import it again."
    ImportResult.NothingToImport -> "No new workouts found in that file."
    ImportResult.UnrecognisedFormat ->
        "That file isn't a recognised gym-app export. Export a CSV from Strong, Hevy, FitNotes, or a similar app, or an Avex JSON export."
    ImportResult.TooLarge -> "That file is too large to import."
    ImportResult.ReadError -> "Couldn't read that file. Try again, or pick a different copy."
}
