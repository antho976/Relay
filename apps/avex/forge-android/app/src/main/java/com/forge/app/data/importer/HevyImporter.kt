package com.forge.app.data.importer

/**
 * Hevy (hevyapp.com) CSV export (#GYMAP-17). One set per row, grouped into workouts by
 * (start_time, title). Hevy writes the account's unit into the header: `weight_kg` / `distance_km`
 * for a metric account, `weight_lbs` / `distance_miles` for an imperial one. Metric header:
 *
 * `title, start_time, end_time, description, exercise_title, superset_id, exercise_notes,
 *  set_index, set_type, weight_kg, reps, distance_km, duration_seconds, rpe`
 */
class HevyImporter : GymImporter {
    override val source = ImportSource.HEVY

    override fun canParse(text: String): Boolean {
        val header = ImportParsing.firstLine(text)
        return header.contains("exercise_title") &&
            (header.contains("weight_kg") || header.contains("weight_lb") || header.contains("start_time"))
    }

    override fun parse(text: String, assumeKg: Boolean): List<ImportedSession> = read(text, assumeKg).sessions

    /** The file's cardio rows (runs, rides, rows), which [parse] leaves out of the workouts. */
    override fun parseExtras(text: String, assumeKg: Boolean): ImportedExtras = read(text, assumeKg).extras

    override fun read(text: String, assumeKg: Boolean): ParsedImport {
        val rows = CsvParser.parse(text)
        if (rows.size < 2) return ParsedImport(emptyList())
        val idx = ImportParsing.headerIndex(rows.first())
        // Resolved from the header, as FitNotes does. Only `weight_kg` used to be read, so an
        // imperial account's export (`weight_lbs`) came in with every weight cell "" — years of
        // history stored as bodyweight sets, volume 0, PRs, goals and trophies empty.
        val weight = weightColumn(idx, assumeKg)
        val distance = distanceColumn(idx, assumeKg)
        val dateOf = ImportParsing.dateReader(rows.asSequence().drop(1).map { ImportParsing.cell(it, idx, "start_time") })

        val sessions = LinkedHashMap<String, WorkingSession>()
        val cardio = ArrayList<ImportedCardio>()
        var skipped = 0
        for (row in rows.drop(1)) {
            val startRaw = ImportParsing.cell(row, idx, "start_time")
            val startedAt = dateOf(startRaw)
            if (startedAt == null) { skipped++; continue }
            val exerciseName = ImportParsing.cell(row, idx, "exercise_title")
            if (exerciseName.isBlank()) { skipped++; continue }

            val reps = ImportParsing.parseReps(ImportParsing.cell(row, idx, "reps"))
            val weightRaw = weight?.let { ImportParsing.parseWeight(ImportParsing.cell(row, idx, it.first)) }
            val durationSeconds = ImportParsing.parseSeconds(ImportParsing.cell(row, idx, "duration_seconds"))
            val distanceKm = distance?.let { (col, unit) ->
                ImportParsing.distanceKm(ImportParsing.cell(row, idx, col), unit, assumeKg)
            }
            ImportParsing.cardioRowType(exerciseName, reps, weightRaw, durationSeconds, distanceKm)?.let { type ->
                cardio.add(ImportedCardio(
                    dateMs = startedAt, type = type,
                    durationMin = ImportParsing.cardioMinutes(durationSeconds), distanceKm = distanceKm
                ))
                continue
            }
            if ((reps ?: 0) <= 0 && (weightRaw == null || weightRaw == 0.0) && durationSeconds == null) { skipped++; continue }

            val weightLb = weightRaw?.takeIf { it > 0.0 }?.let {
                ImportParsing.roundWeight(if (weight?.second == true) ImportParsing.kgToLb(it) else it)
            }
            val rpe = ImportParsing.parseWeight(ImportParsing.cell(row, idx, "rpe"))?.takeIf { it in 1.0..10.0 }
            val setType = ImportParsing.cell(row, idx, "set_type").lowercase()

            val title = ImportParsing.cell(row, idx, "title").ifBlank { "Workout" }
            val key = "$startRaw|$title"
            val session = sessions.getOrPut(key) {
                val endAt = dateOf(ImportParsing.cell(row, idx, "end_time"))
                WorkingSession(
                    startedAt = startedAt,
                    durationMs = endAt?.let { (it - startedAt).takeIf { d -> d > 0 } },
                    title = title,
                    note = ImportParsing.cell(row, idx, "description").ifBlank { null }
                )
            }
            session.exercises.getOrPut(exerciseName) { mutableListOf() }.add(
                ImportedSet(weightLb = weightLb, reps = reps ?: 0, durationSeconds = durationSeconds, rpe = rpe, isWarmup = setType == "warmup")
            )
        }
        return ParsedImport(sessions.values.map { it.toImported() }, ImportedExtras(cardio = cardio), skipped)
    }

    /** The weight column and whether it is kg: `weight_kg`, else `weight_lbs`, else a bare `weight` in [assumeKg]. */
    private fun weightColumn(idx: Map<String, Int>, assumeKg: Boolean): Pair<String, Boolean>? = when {
        "weight_kg" in idx -> "weight_kg" to true
        "weight_lbs" in idx -> "weight_lbs" to false
        "weight_lb" in idx -> "weight_lb" to false
        "weight" in idx -> "weight" to assumeKg
        else -> null
    }

    /** The distance column and the unit it is in. */
    private fun distanceColumn(idx: Map<String, Int>, assumeKg: Boolean): Pair<String, String>? = when {
        "distance_km" in idx -> "distance_km" to "km"
        "distance_miles" in idx -> "distance_miles" to "mi"
        "distance_meters" in idx -> "distance_meters" to "m"
        "distance" in idx -> "distance" to if (assumeKg) "km" else "mi"
        else -> null
    }
}
