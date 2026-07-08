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

    override fun parse(text: String, assumeKg: Boolean): List<ImportedSession> {
        val rows = CsvParser.parse(text)
        if (rows.size < 2) return emptyList()
        val idx = ImportParsing.headerIndex(rows.first())

        val weightCol = ImportParsing.findCol(idx, "weight")
        val weightHeaderKg = idx.keys.firstOrNull { it.contains("weight") && it.contains("kg") } != null
        val weightHeaderLb = idx.keys.firstOrNull { it.contains("weight") && it.contains("lb") } != null
        val unitCol = idx["weight unit"]

        val sessions = LinkedHashMap<String, WorkingSession>()
        for (row in rows.drop(1)) {
            val dateRaw = ImportParsing.cell(row, idx, "date")
            val startedAt = ImportParsing.parseEpochMillis(dateRaw) ?: continue
            val exerciseName = ImportParsing.cell(row, idx, "exercise")
            if (exerciseName.isBlank()) continue

            val reps = ImportParsing.parseReps(ImportParsing.cell(row, idx, "reps"))
            val weightRaw = ImportParsing.parseWeight(ImportParsing.at(row, weightCol))
            if (reps == null && (weightRaw == null || weightRaw == 0.0)) continue

            val kg = ImportParsing.rowIsKg(row, unitCol, weightHeaderKg, weightHeaderLb, assumeKg)
            val weightLb = weightRaw?.takeIf { it > 0.0 }
                ?.let { ImportParsing.roundWeight(if (kg) ImportParsing.kgToLb(it) else it) }

            val session = sessions.getOrPut(dateRaw) {
                WorkingSession(startedAt = startedAt, durationMs = null, title = null)
            }
            session.exercises.getOrPut(exerciseName) { mutableListOf() }
                .add(ImportedSet(weightLb = weightLb, reps = reps ?: 0))
        }
        return sessions.values.map { it.toImported() }
    }
}
