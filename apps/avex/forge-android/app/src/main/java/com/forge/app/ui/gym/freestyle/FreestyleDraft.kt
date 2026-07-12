package com.forge.app.ui.gym.freestyle

import org.json.JSONArray
import org.json.JSONObject

/**
 * One drafted set: the raw display-unit weight text and reps text exactly as typed, plus its set-type
 * tags (GYMAP-46). [setType] is the mutually-exclusive shape (null | "warmup" | "drop"); AMRAP and
 * failure are independent flags; [rpe] is 1.0–10.0 in 0.5 steps or null.
 */
internal data class FreestyleDraftSet(
    val weight: String,
    val reps: String,
    val setType: String? = null,
    val isAmrap: Boolean = false,
    val toFailure: Boolean = false,
    val rpe: Double? = null
)

/** One drafted exercise: a library id + its sets. Name/muscle/bodyweight are re-derived from the
 *  library on restore, so only the id is stored — a move dropped from the library resolves cleanly. */
internal data class FreestyleDraftExercise(val libId: String, val sets: List<FreestyleDraftSet>)

/**
 * A snapshot of an in-progress freestyle log — the exercises/sets typed so far plus when the logger
 * was opened — persisted as one JSON blob so a navigate-away or a full app kill can resume it. Cleared
 * atomically when the workout is saved or explicitly discarded. Mirrors [com.forge.app.ui.onboarding.
 * OnboardingDraft]'s hand-rolled `org.json` round-trip (the app's draft-persistence precedent).
 */
internal data class FreestyleDraft(
    val openedAtMs: Long,
    val exercises: List<FreestyleDraftExercise>
) {
    fun toJson(): String = JSONObject().apply {
        put("schema", SCHEMA)
        put("openedAtMs", openedAtMs)
        put("exercises", JSONArray(exercises.map { ex ->
            JSONObject().apply {
                put("libId", ex.libId)
                put("sets", JSONArray(ex.sets.map { s ->
                    JSONObject().apply {
                        put("w", s.weight)
                        put("r", s.reps)
                        // Tags are written only when set, so an untagged draft stays as compact as before.
                        s.setType?.let { put("t", it) }
                        if (s.isAmrap) put("amrap", true)
                        if (s.toFailure) put("fail", true)
                        s.rpe?.let { put("rpe", it) }
                    }
                }))
            }
        }))
    }.toString()

    companion object {
        /** Bump if the shape changes so a draft written by an older build is discarded, not misread.
         *  v2 (GYMAP-46): per-set type tags (t/amrap/fail/rpe). */
        private const val SCHEMA = 2

        /** Null on any parse failure or a stale schema — the logger just opens empty. */
        fun fromJson(json: String): FreestyleDraft? = runCatching {
            val o = JSONObject(json)
            if (o.optInt("schema", 0) != SCHEMA) return null
            val exArr = o.getJSONArray("exercises")
            val exercises = (0 until exArr.length()).map { i ->
                val exo = exArr.getJSONObject(i)
                val setsArr = exo.getJSONArray("sets")
                FreestyleDraftExercise(
                    libId = exo.getString("libId"),
                    sets = (0 until setsArr.length()).map { j ->
                        val so = setsArr.getJSONObject(j)
                        FreestyleDraftSet(
                            weight = so.optString("w", ""),
                            reps = so.optString("r", ""),
                            setType = if (so.isNull("t")) null else so.optString("t").ifBlank { null },
                            isAmrap = so.optBoolean("amrap", false),
                            toFailure = so.optBoolean("fail", false),
                            rpe = if (so.has("rpe")) so.getDouble("rpe") else null
                        )
                    }
                )
            }
            FreestyleDraft(openedAtMs = o.getLong("openedAtMs"), exercises = exercises)
        }.getOrNull()
    }
}
