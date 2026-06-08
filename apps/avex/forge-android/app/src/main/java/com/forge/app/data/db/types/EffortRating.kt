package com.forge.app.data.db.types

/**
 * How hard the user rates a logged exercise after finishing it. Distinct from
 * [com.forge.app.program.Difficulty] — that one describes the *exercise's inherent*
 * difficulty (beginner/intermediate/advanced), this describes *how the user felt
 * doing it today*.
 *
 * "Brutal" is the trigger for the "Hit the Wall" trophy. The set "Brutal" must
 * remain interpretable forever, so storage is via [code] string, not enum ordinal.
 */
enum class EffortRating(val code: String, val displayName: String) {
    EASY("easy", "Easy"),
    JUST_RIGHT("just-right", "Just Right"),
    HARD("hard", "Hard"),
    BRUTAL("brutal", "Brutal");

    companion object {
        /** Null for an unrecognized/legacy code so a Room read never crashes. */
        fun fromCode(code: String): EffortRating? =
            entries.firstOrNull { it.code == code }
    }
}
