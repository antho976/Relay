package com.forge.app.ui.gym.freestyle

import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.parseToLb
import com.forge.app.domain.units.weightInputValue
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
    val rpe: Double? = null,
    /** Raw hold-time text for a timed-hold set (GYMAP-51), e.g. "1:30"; blank for a rep set. */
    val hold: String = ""
)

/**
 * One drafted exercise: a library id + its sets. For a library move only the id is stored —
 * name/muscle/bodyweight are re-derived on restore, so a move dropped from the library resolves
 * cleanly. A user-created custom move ([customExerciseId]) has no library row to re-derive from, so
 * it carries its own [name] and [muscleCode]; both are null for a library move.
 */
internal data class FreestyleDraftExercise(
    val libId: String,
    val sets: List<FreestyleDraftSet>,
    val name: String? = null,
    val muscleCode: String? = null
)

/**
 * A snapshot of an in-progress freestyle log — the exercises/sets typed so far plus when the logger
 * was opened — persisted as one JSON blob so a navigate-away or a full app kill can resume it. Cleared
 * atomically when the workout is saved or explicitly discarded. Mirrors [com.forge.app.ui.onboarding.
 * OnboardingDraft]'s hand-rolled `org.json` round-trip (the app's draft-persistence precedent).
 */
internal data class FreestyleDraft(
    val openedAtMs: Long,
    val exercises: List<FreestyleDraftExercise>,
    /**
     * The display unit the weight text was typed in ([com.forge.app.domain.units.WeightUnit.label]),
     * or null for a draft written before this was recorded.
     *
     * The sets are stored as raw display-unit text, which is meaningless without knowing which unit
     * that was: draft "100" while set to lb, switch the app to kg, resume, and the same "100" was
     * saved as 100 kg — a 220 lb set, in the history and every aggregate built on it. Null restores
     * verbatim, which is the old behaviour and the only honest answer for a draft that never said.
     */
    val unitLabel: String? = null
) {
    fun toJson(): String = JSONObject().apply {
        put("schema", SCHEMA)
        put("openedAtMs", openedAtMs)
        // Additive and optional, like the per-set tags: an older build ignores "u", and a draft
        // without it reads back as null. No schema bump, so an in-progress log survives the upgrade.
        unitLabel?.let { put("u", it) }
        put("exercises", JSONArray(exercises.map { ex ->
            JSONObject().apply {
                put("libId", ex.libId)
                // Custom-move identity, written only for a custom — a library draft stays as compact as before.
                ex.name?.let { put("name", it) }
                ex.muscleCode?.let { put("muscle", it) }
                put("sets", JSONArray(ex.sets.map { s ->
                    JSONObject().apply {
                        put("w", s.weight)
                        put("r", s.reps)
                        // Tags are written only when set, so an untagged draft stays as compact as before.
                        s.setType?.let { put("t", it) }
                        if (s.isAmrap) put("amrap", true)
                        if (s.toFailure) put("fail", true)
                        s.rpe?.let { put("rpe", it) }
                        // Hold time written only for timed sets — a compatible additive field (old
                        // builds ignore "h"; a draft without it reads hold = "").
                        if (s.hold.isNotBlank()) put("h", s.hold)
                    }
                }))
            }
        }))
    }.toString()

    /**
     * [text] — a raw weight as typed into this draft — expressed in [current].
     *
     * Unchanged when the draft never recorded its unit (an older blob), when the unit has not
     * changed, or when the text is not a number ("BW", blank). Otherwise converted through the
     * shared lb round-trip, so the number the user sees on resume means what it did when they typed
     * it rather than what the setting happens to say now.
     */
    fun weightTextIn(text: String, current: WeightUnit): String {
        val typedIn = unitLabel?.let { label -> WeightUnit.entries.firstOrNull { it.label == label } }
        if (typedIn == null || typedIn == current || text.isBlank()) return text
        val lb = parseToLb(text, typedIn) ?: return text
        return weightInputValue(lb, current)
    }

    companion object {
        /** Bump if the shape changes so a draft written by an older build is discarded, not misread.
         *  v2 (GYMAP-46): per-set type tags (t/amrap/fail/rpe).
         *  v3: custom (user-created) moves carry their own name/muscle. */
        private const val SCHEMA = 3

        /** Versions this build can still read. v3 only ADDED optional per-exercise name/muscle, so a
         *  v2 blob parses identically (no custom moves existed then) — reading it costs nothing and
         *  saves an in-progress log from being dropped on upgrade. */
        private val READABLE = setOf(2, SCHEMA)

        /** Null on any parse failure or a stale schema — the logger just opens empty. */
        fun fromJson(json: String): FreestyleDraft? = runCatching {
            val o = JSONObject(json)
            if (o.optInt("schema", 0) !in READABLE) return null
            val exArr = o.getJSONArray("exercises")
            val exercises = (0 until exArr.length()).map { i ->
                val exo = exArr.getJSONObject(i)
                val setsArr = exo.getJSONArray("sets")
                FreestyleDraftExercise(
                    libId = exo.getString("libId"),
                    name = exo.optString("name").ifBlank { null },
                    muscleCode = exo.optString("muscle").ifBlank { null },
                    sets = (0 until setsArr.length()).map { j ->
                        val so = setsArr.getJSONObject(j)
                        FreestyleDraftSet(
                            weight = so.optString("w", ""),
                            reps = so.optString("r", ""),
                            setType = if (so.isNull("t")) null else so.optString("t").ifBlank { null },
                            isAmrap = so.optBoolean("amrap", false),
                            toFailure = so.optBoolean("fail", false),
                            rpe = if (so.has("rpe")) so.getDouble("rpe") else null,
                            hold = so.optString("h", "")
                        )
                    }
                )
            }
            FreestyleDraft(
                openedAtMs = o.getLong("openedAtMs"),
                // A blob that somehow carries the same move twice must not restore two rows: the
                // logger keys its lazy list on libId, and a duplicate crashes it on every resume.
                exercises = exercises.distinctBy { it.libId },
                unitLabel = o.optString("u").ifBlank { null }
            )
        }.getOrNull()
    }
}
