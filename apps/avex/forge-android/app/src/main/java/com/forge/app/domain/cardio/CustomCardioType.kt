package com.forge.app.domain.cardio

import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

/**
 * A user-defined cardio activity (GYMAP-37) — e.g. "Padel", "Kayaking". Sits beside the built-in
 * [CardioType]s in the log picker and everywhere a session's activity is shown.
 *
 * These definitions live as one JSON blob in DataStore (see [com.forge.app.data.prefs.SettingsRepository]),
 * NOT the Room DB: they're a small user config list with no relational queries, and `cardio_entry.type`
 * is already an open string column, so a logged session just stores this [code] — no schema migration.
 *
 * [code] is `custom_<uuid8>`, generated once at creation and stable forever, so renaming the activity
 * never re-keys existing sessions. [glyphKey] indexes [CardioGlyphs]. Calorie handling is deliberately
 * NOT stored: an unknown code resolves to [CardioType.OTHER] in [CardioType.fromCode], so custom
 * activities inherit "Other"'s calorie behaviour for free (and the app surfaces no kcal estimate today
 * anyway — DESIGN §14).
 */
data class CustomCardioType(
    val code: String,
    val name: String,
    val glyphKey: String = CardioGlyphs.DEFAULT_KEY,
) {
    companion object {
        const val CODE_PREFIX = "custom_"

        /** True for a stored `cardio_entry.type` that refers to a user-defined activity. */
        fun isCustomCode(code: String): Boolean = code.startsWith(CODE_PREFIX)

        /** Max length of a custom activity name — keeps rows/pills single-line. */
        const val MAX_NAME_LEN = 24

        /** Mint a new activity with a fresh stable code. [name] is trimmed/clamped; a blank name is the
         *  caller's responsibility to reject before calling. */
        fun create(name: String, glyphKey: String): CustomCardioType = CustomCardioType(
            code = CODE_PREFIX + UUID.randomUUID().toString().take(8),
            name = name.trim().take(MAX_NAME_LEN),
            glyphKey = glyphKey,
        )

        /** Bump if the persisted shape changes so an older build's blob is discarded, not misread. */
        private const val SCHEMA = 1

        fun listToJson(types: List<CustomCardioType>): String = JSONObject().apply {
            put("schema", SCHEMA)
            put("types", JSONArray(types.map { t ->
                JSONObject().apply {
                    put("code", t.code)
                    put("name", t.name)
                    put("glyph", t.glyphKey)
                }
            }))
        }.toString()

        /** Tolerant parse — null/blank/corrupt/stale-schema all yield an empty list (feature just shows
         *  the built-in types), never a crash. */
        fun listFromJson(json: String?): List<CustomCardioType> {
            if (json.isNullOrBlank()) return emptyList()
            return runCatching {
                val root = JSONObject(json)
                if (root.optInt("schema", -1) != SCHEMA) return emptyList()
                val arr = root.optJSONArray("types") ?: return emptyList()
                buildList {
                    for (i in 0 until arr.length()) {
                        val o = arr.optJSONObject(i) ?: continue
                        val code = o.optString("code").takeIf { it.isNotBlank() } ?: continue
                        val name = o.optString("name").takeIf { it.isNotBlank() } ?: continue
                        add(CustomCardioType(code, name, o.optString("glyph", CardioGlyphs.DEFAULT_KEY)))
                    }
                }
            }.getOrDefault(emptyList())
        }
    }
}
