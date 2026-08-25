package com.forge.app.data.importer

/**
 * Last-resort CSV importer (#GYMAP-17) — matches columns by fuzzy header name (date / exercise /
 * weight / reps, plus optional workout name + unit) rather than an exact schema. This is what lets
 * "every major and somewhat known" app import even without a dedicated parser: Jefit, RepCount,
 * GymBook, Liftin, and hand-made spreadsheets all export a Date/Exercise/Weight/Reps CSV that lands
 * here. Runs only after the dedicated importers decline, so it never steals a Strong/Hevy/FitNotes
 * file it would parse less precisely.
 */
class GenericCsvImporter : GymImporter {
    override val source = ImportSource.GENERIC_CSV

    /**
     * Header-only, like every other importer's sniff. This used to parse the ENTIRE file into cells
     * just to read its column names, throw that away, and let [parse] do it all over again — so an
     * unrecognised 20 MB spreadsheet sitting in Downloads was fully parsed twice on every visit to
     * the Import screen, for nothing.
     */
    override fun canParse(text: String): Boolean =
        CsvParser.parseHeader(text)?.let { columnsFrom(it) } != null

    override fun parse(text: String, assumeKg: Boolean): List<ImportedSession> {
        // Parse the file ONCE and share the rows with column resolution — resolve() used to re-parse,
        // so a generic import scanned the whole file three times (canParse + here + resolve).
        val rows = CsvParser.parse(text)
        val cols = resolve(rows) ?: return emptyList()

        val sessions = LinkedHashMap<String, WorkingSession>()
        for (row in rows.drop(1)) {
            val dateRaw = ImportParsing.at(row, cols.date)
            val startedAt = ImportParsing.parseEpochMillis(dateRaw) ?: continue
            val exerciseName = ImportParsing.at(row, cols.exercise)
            if (exerciseName.isBlank()) continue

            val reps = ImportParsing.parseReps(ImportParsing.at(row, cols.reps))
            val weightRaw = ImportParsing.parseWeight(ImportParsing.at(row, cols.weight))
            if (reps == null && (weightRaw == null || weightRaw == 0.0)) continue

            val kg = ImportParsing.rowIsKg(row, cols.unit, cols.weightHeaderKg, cols.weightHeaderLb, assumeKg)
            val weightLb = weightRaw?.takeIf { it > 0.0 }
                ?.let { ImportParsing.roundWeight(if (kg) ImportParsing.kgToLb(it) else it) }

            val workout = ImportParsing.at(row, cols.workout).ifBlank { "" }
            val key = "$dateRaw|$workout"
            val session = sessions.getOrPut(key) {
                WorkingSession(startedAt = startedAt, durationMs = null, title = workout.ifBlank { null })
            }
            session.exercises.getOrPut(exerciseName) { mutableListOf() }
                .add(ImportedSet(weightLb = weightLb, reps = reps ?: 0))
        }
        return sessions.values.map { it.toImported() }
    }

    private data class Columns(
        val date: Int, val exercise: Int, val weight: Int?, val reps: Int?,
        val workout: Int?, val unit: Int?, val weightHeaderKg: Boolean, val weightHeaderLb: Boolean
    )

    /** Resolve the required columns from already-parsed rows, or null if this isn't a workout CSV. */
    private fun resolve(rows: List<List<String>>): Columns? {
        if (rows.size < 2) return null
        return columnsFrom(rows.first())
    }

    /** The column map derived from the header row alone — all detection ever needed. */
    private fun columnsFrom(header: List<String>): Columns? {
        val idx = ImportParsing.headerIndex(header)
        val date = ImportParsing.findCol(idx, "date") ?: ImportParsing.findCol(idx, "start") ?: return null
        val exercise = ImportParsing.findCol(idx, "exercise", "movement", "lift") ?: return null
        // Exclude a per-row "Bodyweight" column: it holds the user's bodyweight, not the lift load,
        // yet contains the substring "weight" and would otherwise be picked as the weight column.
        val weight = idx.entries
            .firstOrNull { e ->
                !e.key.contains("bodyweight") && !e.key.contains("body weight") &&
                    listOf("weight", "load", "kg", "lbs").any { e.key.contains(it) }
            }?.value
        val reps = ImportParsing.findCol(idx, "reps", "rep")
        // Need at least one of weight/reps to have anything to log.
        if (weight == null && reps == null) return null
        return Columns(
            date = date,
            exercise = exercise,
            weight = weight,
            reps = reps,
            workout = ImportParsing.findCol(idx, "workout", "session", "routine", "title", "day"),
            unit = idx["weight unit"] ?: idx["unit"],
            weightHeaderKg = idx.keys.any { it.contains("weight") && it.contains("kg") && !it.contains("bodyweight") },
            weightHeaderLb = idx.keys.any { it.contains("weight") && it.contains("lb") && !it.contains("bodyweight") }
        )
    }
}
