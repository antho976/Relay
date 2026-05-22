package com.forge.app.domain.cardio

/**
 * Self-rated effort for a non-rest cardio entry. Same string-storage approach as
 * [com.forge.app.data.db.types.EffortRating] in the gym side — the [code] field is
 * the durable identifier, [displayName] is for the UI.
 */
enum class CardioEffort(val code: String, val displayName: String) {
    EASY("easy", "Easy"),
    MODERATE("moderate", "Moderate"),
    HARD("hard", "Hard");

    companion object {
        fun fromCode(code: String?): CardioEffort? =
            code?.let { c -> entries.firstOrNull { it.code == c } }
    }
}

/**
 * Why the user took a rest day. Visible only when [CardioType] is REST. Stored on
 * [com.forge.app.data.db.entities.CardioEntry.restReason].
 */
enum class CardioRestReason(val code: String, val displayName: String) {
    PLANNED("planned", "Planned"),
    SORE("sore", "Sore"),
    SICK("sick", "Sick"),
    BUSY("busy", "Busy");

    companion object {
        fun fromCode(code: String?): CardioRestReason? =
            code?.let { c -> entries.firstOrNull { it.code == c } }
    }
}
