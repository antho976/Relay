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
                        setType = setType
                    ))
                }
                if (sets.isNotEmpty()) {
                    exercises.add(ImportedExercise(
                        name = name, sets = sets,
                        note = ex.optString("note").ifBlank { null }, catalogueId = catalogueId
                    ))
                }
            }
            if (exercises.isNotEmpty()) {
                out.add(
                    ImportedSession(
                        startedAtMs = startedAt,
                        finishedAtMs = finishedAt,
                        exercises = exercises,
                        note = s.optString("journal").ifBlank { null }
                    )
                )
            }
        }
        return out
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
}
