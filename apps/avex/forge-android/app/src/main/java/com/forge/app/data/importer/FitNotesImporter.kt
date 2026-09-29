package com.forge.app.data.importer

/**
 * FitNotes CSV export (#GYMAP-17). One set per row; FitNotes has no workout grouping, so every set
 * on the same Date becomes one session. Two header shapes exist and both are handled:
 *
 * `Date, Exercise, Category, Weight, Weight Unit, Reps, Distance, Distance Unit, Time, Comment`
 * `Date, Exercise, Category, Weight (kgs), Reps, Distance, Distance Unit, Time, Comment`  (unit in
 * the weight header)
 */
class FitNotesImporter : GymImporter {
    override val source = ImportSource.FITNOTES

    override fun canParse(text: String): Boolean {
        val header = ImportParsing.firstLine(text)
        return header.contains("date") && header.contains("exercise") &&
            header.contains("category") &&
            !header.contains("exercise_title") // not Hevy
    }

    override fun parse(text: String, assumeKg: Boolean): List<ImportedSession> = read(text, assumeKg).sessions

    /** The file's cardio rows: a distance, or a time under FitNotes' own Cardio category. */
    override fun parseExtras(text: String, assumeKg: Boolean): ImportedExtras = read(text, assumeKg).extras

    override fun read(text: String, assumeKg: Boolean): ParsedImport {
        val rows = CsvParser.parse(text)
        if (rows.size < 2) return ParsedImport(emptyList())
        val idx = ImportParsing.headerIndex(rows.first())

        // Not a "Bodyweight" or "Weight Unit" column: both contain "weight", and one ahead of the real
        // Weight column was read as the load (the same exclusion GenericCsvImporter makes).
        fun isLoadHeader(h: String) =
            h.contains("weight") && !h.contains("bodyweight") && !h.contains("body weight") && !h.contains("unit")
        val weightCol = idx.entries.firstOrNull { isLoadHeader(it.key) }?.value
        val weightHeaderKg = idx.keys.any { isLoadHeader(it) && it.contains("kg") }
        val weightHeaderLb = idx.keys.any { isLoadHeader(it) && it.contains("lb") }
        val unitCol = idx["weight unit"]
        val dateOf = ImportParsing.dateReader(rows.asSequence().drop(1).map { ImportParsing.cell(it, idx, "date") })

        val sessions = LinkedHashMap<String, WorkingSession>()
        val cardio = ArrayList<ImportedCardio>()
        var skipped = 0
        for (row in rows.drop(1)) {
            val dateRaw = ImportParsing.cell(row, idx, "date")
            val startedAt = dateOf(dateRaw)
            if (startedAt == null) { skipped++; continue }
            val exerciseName = ImportParsing.cell(row, idx, "exercise")
            if (exerciseName.isBlank()) { skipped++; continue }

            val reps = ImportParsing.parseReps(ImportParsing.cell(row, idx, "reps"))
            val weightCell = ImportParsing.at(row, weightCol)
            val weightRaw = ImportParsing.parseWeight(weightCell)
            // `Time` was never read, so a plank (Weight 0, Reps 0, Time) was dropped uncounted.
            val seconds = ImportParsing.parseClockOrSeconds(ImportParsing.cell(row, idx, "time"))
            val distanceKm = ImportParsing.distanceKm(
                ImportParsing.cell(row, idx, "distance"), ImportParsing.cell(row, idx, "distance unit"), assumeKg
            )
            val unloaded = (reps ?: 0) <= 0 && (weightRaw ?: 0.0) <= 0.0
            val cardioType = ImportParsing.cardioRowType(exerciseName, reps, weightRaw, seconds, distanceKm)
                ?: if (unloaded && seconds != null &&
                    ImportParsing.cell(row, idx, "category").equals("cardio", ignoreCase = true)
                ) "other" else null
            if (cardioType != null) {
                cardio.add(ImportedCardio(
                    dateMs = startedAt, type = cardioType,
                    durationMin = ImportParsing.cardioMinutes(seconds), distanceKm = distanceKm
                ))
                continue
            }
            if (unloaded && seconds == null) { skipped++; continue }

            val kg = ImportParsing.rowIsKg(row, unitCol, weightHeaderKg, weightHeaderLb, assumeKg, weightCell)
            val weightLb = weightRaw?.takeIf { it > 0.0 }
                ?.let { ImportParsing.roundWeight(if (kg) ImportParsing.kgToLb(it) else it) }

            val session = sessions.getOrPut(dateRaw) {
                WorkingSession(startedAt = startedAt, durationMs = null, title = null)
            }
            session.exercises.getOrPut(exerciseName) { mutableListOf() }
                .add(ImportedSet(weightLb = weightLb, reps = reps ?: 0, durationSeconds = seconds))
        }
        return ParsedImport(sessions.values.map { it.toImported() }, ImportedExtras(cardio = cardio), skipped)
    }
}
