package com.forge.app.ui.onboarding

import org.json.JSONArray
import org.json.JSONObject

/**
 * A mid-onboarding snapshot — every answer plus the current page — persisted as one JSON blob so a
 * fully killed app resumes setup where it left off (cleared atomically on completion). Nullable
 * fields keep their tri-state through the round-trip: [sex] null = not asked yet ("" = explicitly
 * "prefer not to say"), [frozenIds] null = no curated preset locked.
 */
internal data class OnboardingDraft(
    val page: Int,
    val planMode: String,
    val name: String,
    val useKg: Boolean,
    val useMilesChoice: Boolean,
    val distanceTouched: Boolean,
    val goal: String,
    val experience: String,
    val bodyweightInput: String,
    val sex: String?,
    val daysPerWeek: Int,
    val equipment: Set<String>,
    val frozenIds: Set<String>?,
    val plateWeightLb: Double,
    val problemAreas: Set<String>,
    val cadence: String,
    val everyN: Int,
    val previewSeed: Long
) {
    fun toJson(): String = JSONObject().apply {
        put("page", page)
        put("planMode", planMode)
        put("name", name)
        put("useKg", useKg)
        put("useMilesChoice", useMilesChoice)
        put("distanceTouched", distanceTouched)
        put("goal", goal)
        put("experience", experience)
        put("bodyweightInput", bodyweightInput)
        sex?.let { put("sex", it) }                       // absent = never picked
        put("daysPerWeek", daysPerWeek)
        put("equipment", JSONArray(equipment.toList()))
        frozenIds?.let { put("frozenIds", JSONArray(it.toList())) }
        put("plateWeightLb", plateWeightLb)
        put("problemAreas", JSONArray(problemAreas.toList()))
        put("cadence", cadence)
        put("everyN", everyN)
        put("previewSeed", previewSeed)
    }.toString()

    companion object {
        /** Null on any parse failure — a corrupt draft just restarts onboarding cleanly. */
        fun fromJson(json: String): OnboardingDraft? = runCatching {
            val o = JSONObject(json)
            OnboardingDraft(
                page = o.getInt("page"),
                planMode = o.getString("planMode"),
                name = o.getString("name"),
                useKg = o.getBoolean("useKg"),
                useMilesChoice = o.getBoolean("useMilesChoice"),
                distanceTouched = o.getBoolean("distanceTouched"),
                goal = o.getString("goal"),
                experience = o.getString("experience"),
                bodyweightInput = o.getString("bodyweightInput"),
                sex = if (o.has("sex")) o.getString("sex") else null,
                daysPerWeek = o.getInt("daysPerWeek"),
                equipment = o.getJSONArray("equipment").toStringSet(),
                frozenIds = if (o.has("frozenIds")) o.getJSONArray("frozenIds").toStringSet() else null,
                plateWeightLb = o.getDouble("plateWeightLb"),
                problemAreas = o.getJSONArray("problemAreas").toStringSet(),
                cadence = o.getString("cadence"),
                everyN = o.getInt("everyN"),
                previewSeed = o.getLong("previewSeed")
            )
        }.getOrNull()

        private fun JSONArray.toStringSet(): Set<String> =
            (0 until length()).mapTo(mutableSetOf()) { getString(it) }
    }
}
