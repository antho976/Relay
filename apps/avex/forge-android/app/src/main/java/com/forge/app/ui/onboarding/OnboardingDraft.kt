package com.forge.app.ui.onboarding

import org.json.JSONArray
import org.json.JSONObject

/**
 * A mid-onboarding snapshot — every answer plus the current step — persisted as one JSON blob so a
 * fully killed app resumes setup where it left off (cleared atomically on completion). Nullable
 * fields keep their tri-state through the round-trip: [sex] null = not asked yet ("" = explicitly
 * "prefer not to say"), [frozenIds] null = no curated preset locked, [coachChoice] null = the coach
 * toggle was never touched, so the plan mode's own default still stands.
 */
internal data class OnboardingDraft(
    /** Cursor into the plan mode's path, not a page id — see `pathFor` in OnboardingScreen.kt. */
    val step: Int,
    val planMode: String,
    val name: String,
    val useKg: Boolean,
    val useMilesChoice: Boolean,
    val distanceTouched: Boolean,
    val goal: String,
    val experience: String,
    val bodyweightInput: String,
    val sex: String?,
    /** WearableBrand key ("" = not picked yet). */
    val wearable: String,
    val daysPerWeek: Int,
    val equipment: Set<String>,
    val frozenIds: Set<String>?,
    val plateWeightLb: Double,
    val problemAreas: Set<String>,
    val cadence: String,
    val everyN: Int,
    val previewSeed: Long,
    /** App-lock opt-in (GYMAP-69). */
    val appLock: Boolean,
    /** Explicit coach opt-in / opt-out; null = untouched, so the mode's default applies. */
    val coachChoice: Boolean?
) {
    fun toJson(): String = JSONObject().apply {
        put("schema", SCHEMA)
        put("step", step)
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
        coachChoice?.let { put("coachChoice", it) }       // absent = never touched
    }.toString()

    companion object {
        /** Bump whenever the flow's shape changes so a draft written by an older build — whose
         *  cursor now points at a different step — is discarded rather than resumed mid-flow onto
         *  the wrong screen. The answer fields are name-keyed and would survive, but the cursor
         *  wouldn't. 4 = the 2026-08-22 rebuild, which also renamed `page` to `step`. */
        private const val SCHEMA = 4

        /** Null on any parse failure or a stale schema — the draft just restarts onboarding cleanly. */
        fun fromJson(json: String): OnboardingDraft? = runCatching {
            val o = JSONObject(json)
            if (o.optInt("schema", 1) != SCHEMA) return null
            OnboardingDraft(
                step = o.getInt("step"),
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
                appLock = o.optBoolean("appLock", false),
                coachChoice = if (o.has("coachChoice")) o.getBoolean("coachChoice") else null
            )
        }.getOrNull()

        private fun JSONArray.toStringSet(): Set<String> =
            (0 until length()).mapTo(mutableSetOf()) { getString(it) }
    }
}
