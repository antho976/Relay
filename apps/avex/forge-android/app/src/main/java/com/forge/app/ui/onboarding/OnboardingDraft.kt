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
    /** WearableBrand key ("" = not picked yet) — absent in pre-wearable-step drafts, parsed as "". */
    val wearable: String,
    val daysPerWeek: Int,
    val equipment: Set<String>,
    val frozenIds: Set<String>?,
    val plateWeightLb: Double,
    val problemAreas: Set<String>,
    val cadence: String,
    val everyN: Int,
    val previewSeed: Long,
    /** App-lock opt-in (GYMAP-69) — absent in older drafts, parsed as false. */
    val appLock: Boolean
) {
    fun toJson(): String = JSONObject().apply {
        put("schema", SCHEMA)
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
        put("wearable", wearable)
        put("daysPerWeek", daysPerWeek)
        put("equipment", JSONArray(equipment.toList()))
        frozenIds?.let { put("frozenIds", JSONArray(it.toList())) }
        put("plateWeightLb", plateWeightLb)
        put("problemAreas", JSONArray(problemAreas.toList()))
        put("cadence", cadence)
        put("everyN", everyN)
        put("previewSeed", previewSeed)
        put("appLock", appLock)
    }.toString()

    companion object {
        /** Bump whenever the page layout (indices) changes so a draft written by an older build — whose
         *  `page` cursor now points at a different step — is discarded rather than resumed mid-flow onto
         *  the wrong screen. The answer fields are name-keyed and would survive, but the cursor wouldn't. */
        private const val SCHEMA = 3

        /** Null on any parse failure or a stale schema — the draft just restarts onboarding cleanly. */
        fun fromJson(json: String): OnboardingDraft? = runCatching {
            val o = JSONObject(json)
            if (o.optInt("schema", 1) != SCHEMA) return null
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
                wearable = o.optString("wearable", ""),
                daysPerWeek = o.getInt("daysPerWeek"),
                equipment = o.getJSONArray("equipment").toStringSet(),
                frozenIds = if (o.has("frozenIds")) o.getJSONArray("frozenIds").toStringSet() else null,
                plateWeightLb = o.getDouble("plateWeightLb"),
                problemAreas = o.getJSONArray("problemAreas").toStringSet(),
                cadence = o.getString("cadence"),
                everyN = o.getInt("everyN"),
                previewSeed = o.getLong("previewSeed"),
                appLock = o.optBoolean("appLock", false)
            )
        }.getOrNull()

        private fun JSONArray.toStringSet(): Set<String> =
            (0 until length()).mapTo(mutableSetOf()) { getString(it) }
    }
}
