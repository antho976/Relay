package com.forge.app.domain.mood

/**
 * Post-workout self-check, 5-level scale. Persisted as [code] on
 * [com.forge.app.data.db.entities.MoodEntry.mood] — same string-storage approach
 * as [com.forge.app.data.db.types.EffortRating] so the value stays interpretable
 * forever, even if the enum shifts.
 */
enum class Mood(val code: String, val displayName: String, val emoji: String) {
    DRAINED("drained", "Drained", "😩"),   // 😩
    OFF("off",         "Off",      "😕"),   // 😕
    FINE("fine",       "Fine",     "😐"),   // 😐
    GOOD("good",       "Good",     "🙂"),   // 🙂
    STRONG("strong",   "Strong",   "💪");   // 💪

    companion object {
        fun fromCode(code: String?): Mood? =
            code?.let { c -> entries.firstOrNull { it.code == c } }
    }
}
