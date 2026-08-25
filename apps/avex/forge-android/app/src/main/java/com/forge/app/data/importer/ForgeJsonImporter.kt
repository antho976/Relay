package com.forge.app.data.importer

import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.Program
import org.json.JSONObject
import java.time.LocalDate
import java.time.ZoneId

/**
 * Imports Avex's own JSON export (#GYMAP-17), including new `avex_*.json` files and legacy
 * `forge_*.json` files. Parsing is content-based, so the branding rename does not strand an older
 * export. These exports are explicitly lossy and one-way (nothing read them back before), so this
 * re-imports the sessions/exercises/sets they DO contain: it lets a user move history between two
 * installs via the plain export, alongside the authoritative .zip backup. Weights are already stored
 * in pounds, so no unit conversion happens here.
 */
class ForgeJsonImporter : GymImporter {
    override val source = ImportSource.FORGE_JSON

    /**
     * A cheap sniff, deliberately not a parse. Building the whole document as a JSONObject just to
     * test for one key doubled the peak memory of an import: org.json allocates a HashMap and boxed
     * values per field, so a pretty-printed multi-year export runs 5-10x the text size as a tree,
     * and [parse] immediately built a second one. On a power user's ~12 MB export that was enough to
     * OutOfMemoryError on a modest device — and because the caller's runCatching catches Throwable,
     * the failure surfaced as "No new workouts found in that file", so the user concluded their
     * export was empty and abandoned the migration.
     */
    override fun canParse(text: String): Boolean {
        val t = text.trimStart()
        if (!t.startsWith("{")) return false
        // The key is written near the front by exportFullDataJson, but scan generously rather than
        // depending on key order.
        return t.contains("\"sessions\"")
    }

    /**
     * `exportVersion` as written by `BackupRepository.exportFullDataJson`. Absent in the weekly
     * export and in pre-versioning files, which are format 1 by construction — this only exists to
     * catch a file from a FUTURE Avex whose meaning we'd otherwise guess at.
     */
    override fun formatVersion(text: String): Int? = versionOf(text)

    override fun parse(text: String, assumeKg: Boolean): List<ImportedSession> {
        // Only a malformed document is "no sessions here". An OutOfMemoryError from building the
        // tree is a different fact and must reach the caller, which reports it as a file too large
        // to import rather than as an empty one.
        val root = try {
            JSONObject(text)
        } catch (e: org.json.JSONException) {
            return emptyList()
        }
        val sessionsArr = root.optJSONArray("sessions") ?: return emptyList()
        val out = ArrayList<ImportedSession>(sessionsArr.length())
        for (i in 0 until sessionsArr.length()) {
            val s = sessionsArr.optJSONObject(i) ?: continue
            val startedAt = sessionStartMillis(s) ?: continue
            val finishedAt = s.optLong("finishedAt", 0L).takeIf { it > 0L }
            // Real training time, kept SEPARATE from the wall clock. The full export writes seconds;
            // the weekly export writes whole minutes. Either beats deriving it from finishedAt −
            // startedAt, which counts the away-time of a session resumed the next day.
            val activeSeconds = s.optInt("activeSeconds", 0).takeIf { it > 0 }
                ?: s.optInt("activeMin", 0).takeIf { it > 0 }?.times(60)

            val exArr = s.optJSONArray("exercises") ?: continue
            val exercises = ArrayList<ImportedExercise>(exArr.length())
            for (j in 0 until exArr.length()) {
                val ex = exArr.optJSONObject(j) ?: continue
                val name = exerciseName(ex)
                // Our own export already carries the catalogue id — keep it so a re-import re-links to
                // the same movement instead of guessing from the display name (only real library ids
                // count; custom/legacy ids fall back to name matching).
                val catalogueId = ex.optString("exerciseId").ifBlank { null }
                    ?.takeIf { ExerciseLibrary.byId(it) != null }
                val setArr = ex.optJSONArray("sets") ?: continue
                // The weekly export spells effort "effort"; the full export spells it "difficulty".
                val difficulty = ex.optString("difficulty").ifBlank { null }
                    ?: ex.optString("effort").ifBlank { null }
                val sets = ArrayList<ImportedSet>(setArr.length())
                for (k in 0 until setArr.length()) {
                    val set = setArr.optJSONObject(k) ?: continue
                    val weightLb = set.optDouble("weightLb", 0.0).takeIf { it > 0.0 }
                    val reps = set.optInt("reps", 0)
                    val rpe = set.optDouble("rpe", 0.0).takeIf { it in 1.0..10.0 }
                    // Read back everything that changes what the set means. Older exports simply
                    // don't carry these keys and fall through to the same defaults as before.
                    val setType = set.optString("setType").ifBlank { null }
                    sets.add(ImportedSet(
                        weightLb = weightLb,
                        reps = reps,
                        rpe = rpe,
                        isWarmup = setType == "warmup",
                        durationSeconds = set.optInt("durationSeconds", 0).takeIf { it > 0 },
                        isAssisted = set.optBoolean("isAssisted", false),
                        isAmrap = set.optBoolean("isAmrap", false),
                        toFailure = set.optBoolean("toFailure", false),
                        setType = setType,
                        difficultyTag = set.optString("difficultyTag").ifBlank { null },
                        dropAnnotation = set.optString("dropAnnotation").ifBlank { null },
                        completedAtMs = set.optLong("completedAt", 0L).takeIf { it > 0L }
                    ))
                }
                if (sets.isNotEmpty()) {
                    exercises.add(ImportedExercise(
                        name = name, sets = sets,
                        note = ex.optString("note").ifBlank { null }, catalogueId = catalogueId,
                        orderIndex = ex.optInt("orderIndex", -1).takeIf { it >= 0 },
                        difficulty = difficulty,
                        skipped = ex.optBoolean("skipped", false)
                    ))
                }
            }
            if (exercises.isNotEmpty()) {
                out.add(
                    ImportedSession(
                        startedAtMs = startedAt,
                        finishedAtMs = finishedAt,
                        exercises = exercises,
                        note = s.optString("journal").ifBlank { null },
                        activeSeconds = activeSeconds,
                        // Everything below is what the export has always written and nothing ever
                        // read back: a migrated workout arrived as an untagged, moodless freestyle
                        // session of type "normal" with PR count 0, whatever it had actually been.
                        dayKey = s.optString("dayKey").ifBlank { null },
                        sessionType = s.optString("sessionType").ifBlank { null },
                        intensity = s.optString("intensity").ifBlank { null },
                        isUntracked = s.optBoolean("isUntracked", false),
                        tags = s.optString("tags").ifBlank { null },
                        mood = s.optString("mood").ifBlank { null },
                        prCount = s.optInt("prCount", -1).takeIf { it >= 0 }
                    )
                )
            }
        }
        return out
    }

    /**
     * Cardio entries and coach goals. Both arrays have been written by every full export since they
     * existed and read by nobody: a user migrating via the JSON export lost every cardio session and
     * every goal, and the summary line ("Imported 612 workouts · 24,918 sets") never mentioned it.
     */
    override fun parseExtras(text: String): ImportedExtras {
        val root = try {
            JSONObject(text)
        } catch (e: org.json.JSONException) {
            return ImportedExtras()
        }
        val cardio = ArrayList<ImportedCardio>()
        root.optJSONArray("cardio")?.let { arr ->
            for (i in 0 until arr.length()) {
                val c = arr.optJSONObject(i) ?: continue
                val date = dateMillis(c) ?: continue
                val type = c.optString("type")
                if (type.isBlank()) continue
                cardio.add(
                    ImportedCardio(
                        dateMs = date,
                        type = type,
                        durationMin = c.optInt("durationMin", 0),
                        distanceKm = c.optDouble("distanceKm", 0.0).takeIf { it > 0.0 },
                        effort = c.optString("effort").ifBlank { null },
                        restReason = c.optString("restReason").ifBlank { null },
                        note = c.optString("note").ifBlank { null },
                        inclinePct = c.optDouble("inclinePct", 0.0).takeIf { it > 0.0 },
                        laps = c.optInt("laps", 0).takeIf { it > 0 },
                        elevationM = c.optDouble("elevationM", 0.0).takeIf { it != 0.0 }
                    )
                )
            }
        }
        val goals = ArrayList<ImportedCoachGoal>()
        root.optJSONArray("coachGoals")?.let { arr ->
            for (i in 0 until arr.length()) {
                val g = arr.optJSONObject(i) ?: continue
                val kind = g.optString("kind")
                if (kind.isBlank()) continue
                goals.add(
                    ImportedCoachGoal(
                        kind = kind,
                        targetKey = g.optString("targetKey"),
                        targetValue = g.optDouble("targetValue", Double.NaN).takeIf { !it.isNaN() },
                        priority = g.optInt("priority", 0),
                        createdAt = g.optLong("createdAt", 0L),
                        completedAt = g.optLong("completedAt", 0L).takeIf { it > 0L },
                        archivedAt = g.optLong("archivedAt", 0L).takeIf { it > 0L },
                        source = g.optString("source").ifBlank { "user" },
                        note = g.optString("note")
                    )
                )
            }
        }
        return ImportedExtras(cardio = cardio, coachGoals = goals)
    }

    /**
     * `exportVersion` without building the tree — the same cheap-sniff reasoning as [canParse]. The
     * value is written near the front of the document; a file that doesn't declare one predates
     * versioning (or is the weekly export) and is format 1.
     */
    private fun versionOf(text: String): Int? =
        VERSION_RE.find(text.take(VERSION_SCAN_CHARS))?.groupValues?.getOrNull(1)?.toIntOrNull()

    /** Cardio dates are epoch millis in the full export and a `yyyy-MM-dd` string in the weekly one. */
    private fun dateMillis(c: JSONObject): Long? {
        c.optLong("date", 0L).takeIf { it > 0L }?.let { return it }
        val date = c.optString("date").ifBlank { return null }
        return runCatching {
            LocalDate.parse(date).atStartOfDay(ZoneId.systemDefault()).toInstant().toEpochMilli()
        }.getOrNull()
    }

    /** Full export uses epoch `startedAt`; the weekly export uses a `date` string. */
    private fun sessionStartMillis(s: JSONObject): Long? {
        s.optLong("startedAt", 0L).takeIf { it > 0L }?.let { return it }
        val date = s.optString("date").ifBlank { return null }
        return runCatching {
            LocalDate.parse(date).atStartOfDay(ZoneId.systemDefault()).toInstant().toEpochMilli()
        }.getOrNull()
    }

    /** Weekly export carries a resolved `name`; the full export carries `exerciseId` + `swappedName`. */
    private fun exerciseName(ex: JSONObject): String {
        ex.optString("name").ifBlank { null }?.let { return it }
        val id = ex.optString("exerciseId")
        val swapped = ex.optString("swappedName").ifBlank { null }
        return Program.exerciseDisplayName(id, swapped)
    }

    private companion object {
        val VERSION_RE = Regex("\"exportVersion\"\\s*:\\s*(\\d+)")
        /** The key is written first by `exportFullDataJson`; don't scan a 25 MB document for it. */
        const val VERSION_SCAN_CHARS = 4096
    }
}
