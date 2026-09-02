package com.forge.app.program

import org.json.JSONArray
import org.json.JSONObject

/**
 * A user-created freestyle move (a "custom exercise", id prefix `custom-`): the id the logged rows
 * carry, the name the user typed, and the target muscle they picked when creating it.
 *
 * The logged row stores only the id (plus the name as `swapped_name`); the MUSCLE had nowhere to
 * live, so a saved custom move lost its classification the moment the logger closed: muscle stats
 * and the anatomy figure skipped its sets, and reusing the workout defaulted the move to Chest.
 * Room's schema is frozen here, so like [com.forge.app.domain.cardio.CustomCardioType] these live
 * as one JSON blob in DataStore (see `SettingsRepository.customExercises`), keyed by the id.
 */
data class CustomExerciseDef(
    val id: String,
    val name: String,
    val muscleCode: String
) {
    val muscle: MuscleGroup? get() = MuscleGroup.fromCode(muscleCode)

    companion object {
        /** Bump if the persisted shape changes so an older build's blob is discarded, not misread. */
        private const val SCHEMA = 1

        fun listToJson(defs: Collection<CustomExerciseDef>): String = JSONObject().apply {
            put("schema", SCHEMA)
            put("exercises", JSONArray(defs.map { d ->
                JSONObject().apply {
                    put("id", d.id)
                    put("name", d.name)
                    put("muscle", d.muscleCode)
                }
            }))
        }.toString()

        /** Tolerant parse: null/blank/corrupt/stale-schema all yield an empty list, never a crash. */
        fun listFromJson(json: String?): List<CustomExerciseDef> {
            if (json.isNullOrBlank()) return emptyList()
            return runCatching {
                val root = JSONObject(json)
                if (root.optInt("schema", -1) != SCHEMA) return emptyList()
                val arr = root.optJSONArray("exercises") ?: return emptyList()
                buildList {
                    for (i in 0 until arr.length()) {
                        val o = arr.optJSONObject(i) ?: continue
                        val id = o.optString("id").takeIf { it.isNotBlank() } ?: continue
                        val name = o.optString("name").takeIf { it.isNotBlank() } ?: continue
                        val muscle = o.optString("muscle").takeIf { it.isNotBlank() } ?: continue
                        add(CustomExerciseDef(id, name, muscle))
                    }
                }
            }.getOrDefault(emptyList())
        }
    }
}

/**
 * Process-wide read of the user's custom exercises, the way [Program] is the process-wide read of
 * the active split: loaded from DataStore at app start (ForgeApp) and updated on every write, so
 * the synchronous resolvers ([Program.exercise], [Program.exerciseDisplayName]) can answer for a
 * custom id without threading a repository through every aggregation.
 */
object CustomExerciseRegistry {
    @Volatile
    private var byId: Map<String, CustomExerciseDef> = emptyMap()

    val all: Collection<CustomExerciseDef> get() = byId.values

    /** Replace the whole registry (the DataStore flow's latest value). */
    fun setAll(defs: Collection<CustomExerciseDef>) {
        byId = defs.associateBy { it.id }
    }

    /** Add or replace one definition (a create/save landing before the flow re-emits). */
    fun put(def: CustomExerciseDef) {
        byId = byId + (def.id to def)
    }

    fun get(id: String): CustomExerciseDef? = byId[id]

    fun name(id: String): String? = byId[id]?.name

    fun muscle(id: String): MuscleGroup? = byId[id]?.muscle

    /**
     * The custom move as an [ExercisePlan], so everything that folds sets onto a muscle through
     * [Program.exercise] counts it. Null when the id is unknown or its muscle code no longer parses.
     * Sets/reps/unit are nominal: a custom move has no prescription, only an identity.
     */
    fun plan(id: String): ExercisePlan? {
        val def = byId[id] ?: return null
        val muscle = def.muscle ?: return null
        return ExercisePlan(
            id = def.id,
            name = def.name,
            sets = 3,
            reps = "8-12",
            unit = ExerciseUnit.WEIGHT,
            muscle = muscle,
            difficulty = Difficulty.BEGINNER,
            note = ""
        )
    }

    /** Tests only: forget everything so one test's registration cannot leak into the next. */
    fun clear() {
        byId = emptyMap()
    }
}
