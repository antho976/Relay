package com.forge.app.data.importer

/**
 * Hevy (hevyapp.com) CSV export (#GYMAP-17). One set per row, grouped into workouts by
 * (start_time, title). Weight is always metric (`weight_kg`). Header:
 *
 * `title, start_time, end_time, description, exercise_title, superset_id, exercise_notes,
 *  set_index, set_type, weight_kg, reps, distance_km, duration_seconds, rpe`
 */
class HevyImporter : GymImporter {
    override val source = ImportSource.HEVY

    override fun canParse(text: String): Boolean {
        val header = ImportParsing.firstLine(text)
        return header.contains("exercise_title") &&
            (header.contains("weight_kg") || header.contains("start_time"))
    }

    override fun parse(text: String, assumeKg: Boolean): List<ImportedSession> {
        val rows = CsvParser.parse(text)
        if (rows.size < 2) return emptyList()
        val idx = ImportParsing.headerIndex(rows.first())

        val sessions = LinkedHashMap<String, WorkingSession>()
        for (row in rows.drop(1)) {
            val startRaw = ImportParsing.cell(row, idx, "start_time")
            val startedAt = ImportParsing.parseEpochMillis(startRaw) ?: continue
            val exerciseName = ImportParsing.cell(row, idx, "exercise_title")
            if (exerciseName.isBlank()) continue

            val reps = ImportParsing.parseReps(ImportParsing.cell(row, idx, "reps"))
            val weightKg = ImportParsing.parseWeight(ImportParsing.cell(row, idx, "weight_kg"))
            if (reps == null && (weightKg == null || weightKg == 0.0)) continue

            val weightLb = weightKg?.takeIf { it > 0.0 }?.let { ImportParsing.roundWeight(ImportParsing.kgToLb(it)) }
            val rpe = ImportParsing.parseWeight(ImportParsing.cell(row, idx, "rpe"))?.takeIf { it in 1.0..10.0 }
            val setType = ImportParsing.cell(row, idx, "set_type").lowercase()

            val title = ImportParsing.cell(row, idx, "title").ifBlank { "Workout" }
            val key = "$startRaw|$title"
            val session = sessions.getOrPut(key) {
                val endAt = ImportParsing.parseEpochMillis(ImportParsing.cell(row, idx, "end_time"))
                WorkingSession(
                    startedAt = startedAt,
                    durationMs = endAt?.let { (it - startedAt).takeIf { d -> d > 0 } },
                    title = title,
                    note = ImportParsing.cell(row, idx, "description").ifBlank { null }
                )
            }
            session.exercises.getOrPut(exerciseName) { mutableListOf() }.add(
                ImportedSet(weightLb = weightLb, reps = reps ?: 0, rpe = rpe, isWarmup = setType == "warmup")
            )
        }
        return sessions.values.map { it.toImported() }
    }
}
