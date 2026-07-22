package com.forge.app.domain.cardio

/**
 * Weather / environment tags the user can attach to a cardio session (GYMAP-39) — the conditions the
 * work was done in, so a slow outdoor run reads in context ("15 min · hot · wind") rather than as a
 * bad day. Multi-select and purely descriptive; never affects any total or pace calculation.
 *
 * Stored on [com.forge.app.data.db.entities.CardioEntry.conditions] as a comma-joined list of [code]s
 * (the same durable-string approach as effort / rest reason), null when none were tagged. [code] is
 * the stable identifier; [displayName] is for the UI.
 */
enum class CardioCondition(val code: String, val displayName: String) {
    HOT("hot", "Hot"),
    COLD("cold", "Cold"),
    RAIN("rain", "Rain"),
    WIND("wind", "Wind");

    companion object {
        fun fromCode(code: String?): CardioCondition? =
            code?.let { c -> entries.firstOrNull { it.code == c } }

        /**
         * Decode the stored comma-joined codes into conditions, in declaration order (blank / unknown
         * codes dropped). Declaration order — not the stored order — keeps the chips and the detail
         * line reading consistently (Hot · Cold · Rain · Wind) regardless of tap order.
         */
        fun decode(stored: String?): Set<CardioCondition> {
            if (stored.isNullOrBlank()) return emptySet()
            val codes = stored.split(',').mapNotNullTo(HashSet()) { fromCode(it.trim()) }
            return entries.filterTo(LinkedHashSet()) { it in codes }
        }

        /** Encode a selection to the stored form (declaration order); null when empty so the column stays null. */
        fun encode(conditions: Set<CardioCondition>): String? =
            entries.filter { it in conditions }.joinToString(",") { it.code }.ifEmpty { null }
    }
}
