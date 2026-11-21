package com.forge.app.data.importer

/**
 * Strong (strong.app) CSV export (#GYMAP-17). One of the most common lifting apps. Rows are one set
 * each, grouped into workouts by (Date, Workout Name). Columns vary by version; the constants below
 * are matched case-insensitively and missing ones are tolerated:
 *
 * `Date, Workout Name, Duration, Exercise Name, Set Order, Weight, Weight Unit, Reps, RPE,
 *  Distance, Distance Unit, Seconds, Notes, Workout Notes`
 *
 * Older exports drop `Weight Unit` (the number is then in the user's app unit → [assumeKg]) and use
 * `;` delimiters (handled by [CsvParser]).
 */
class StrongImporter : GymImporter {
    override val source = ImportSource.STRONG

    override fun canParse(text: String): Boolean {
        val header = ImportParsing.firstLine(text)
        return header.contains("workout name") &&
            header.contains("exercise name") &&
            header.contains("set order")
    }

    override fun parse(text: String, assumeKg: Boolean): List<ImportedSession> = read(text, assumeKg).sessions

    /** The file's cardio rows (runs, rides, rows), which [parse] leaves out of the workouts. */
    override fun parseExtras(text: String, assumeKg: Boolean): ImportedExtras = read(text, assumeKg).extras

    override fun read(text: String, assumeKg: Boolean): ParsedImport {
        val rows = CsvParser.parse(text)
        if (rows.size < 2) return ParsedImport(emptyList())
        val idx = ImportParsing.headerIndex(rows.first())
        val hasUnitCol = idx.containsKey("weight unit")
        val dateOf = ImportParsing.dateReader(rows.asSequence().drop(1).map { ImportParsing.cell(it, idx, "date") })

        // (date + workout name) → session; within it, exercise name → its sets, both insertion-ordered.
        val sessions = LinkedHashMap<String, WorkingSession>()
        val cardio = ArrayList<ImportedCardio>()
        var skipped = 0
        for (row in rows.drop(1)) {
            // Newer exports interleave the rest timer as its own row, Seconds holding the rest.
            // It is not a set, and read as one it became a timed hold of the rest length.
            if (ImportParsing.cell(row, idx, "set order").equals("rest timer", ignoreCase = true)) continue
            val dateRaw = ImportParsing.cell(row, idx, "date")
            val startedAt = dateOf(dateRaw)
            if (startedAt == null) { skipped++; continue }
            val exerciseName = ImportParsing.cell(row, idx, "exercise name")
            if (exerciseName.isBlank()) { skipped++; continue }

            val reps = ImportParsing.parseReps(ImportParsing.cell(row, idx, "reps"))
            val weightRaw = ImportParsing.parseWeight(ImportParsing.cell(row, idx, "weight"))
            val durationSeconds = ImportParsing.parseSeconds(ImportParsing.cell(row, idx, "seconds"))
            val distanceKm = ImportParsing.distanceKm(
                ImportParsing.cell(row, idx, "distance"), ImportParsing.cell(row, idx, "distance unit"), assumeKg
            )
            ImportParsing.cardioRowType(exerciseName, reps, weightRaw, durationSeconds, distanceKm)?.let { type ->
                cardio.add(ImportedCardio(
                    dateMs = startedAt, type = type,
                    durationMin = ImportParsing.cardioMinutes(durationSeconds), distanceKm = distanceKm
                ))
                continue
            }
            // Rows with neither reps, weight nor a hold time carry nothing to log.
            if ((reps ?: 0) <= 0 && (weightRaw == null || weightRaw == 0.0) && durationSeconds == null) { skipped++; continue }

            val kg = if (hasUnitCol)
                ImportParsing.cell(row, idx, "weight unit").lowercase().startsWith("kg")
            else assumeKg
            val weightLb = weightRaw?.takeIf { it > 0.0 }
                ?.let { ImportParsing.roundWeight(if (kg) ImportParsing.kgToLb(it) else it) }
            val rpe = ImportParsing.parseWeight(ImportParsing.cell(row, idx, "rpe"))?.takeIf { it in 1.0..10.0 }

            val workoutName = ImportParsing.cell(row, idx, "workout name").ifBlank { "Workout" }
            val key = "$dateRaw|$workoutName"
            val session = sessions.getOrPut(key) {
                WorkingSession(
                    startedAt = startedAt,
                    durationMs = ImportParsing.parseDurationToMillis(ImportParsing.cell(row, idx, "duration")),
                    title = workoutName
                )
            }
            session.exercises.getOrPut(exerciseName) { mutableListOf() }
                .add(ImportedSet(weightLb = weightLb, reps = reps ?: 0, durationSeconds = durationSeconds, rpe = rpe))
        }
        return ParsedImport(sessions.values.map { it.toImported() }, ImportedExtras(cardio = cardio), skipped)
    }
}

/** Mutable accumulator shared by the CSV importers while grouping set rows into workouts. */
internal class WorkingSession(
    val startedAt: Long,
    val durationMs: Long?,
    val title: String?,
    val note: String? = null
) {
    val exercises = LinkedHashMap<String, MutableList<ImportedSet>>()

    fun toImported(): ImportedSession = ImportedSession(
        startedAtMs = startedAt,
        finishedAtMs = durationMs?.let { startedAt + it },
        title = title,
        note = note,
        exercises = exercises.map { (name, sets) -> ImportedExercise(name = name, sets = sets) }
    )
}
