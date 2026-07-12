package com.forge.app.domain.cardio

import androidx.compose.ui.graphics.vector.ImageVector

/**
 * A selectable / displayable cardio activity — either a built-in [CardioType] or a user-defined
 * [CustomCardioType] (GYMAP-37). Unifies the two so the log picker, the entry rows and the session
 * detail all speak one type and don't branch on "is this custom" at every call site.
 *
 * Persistence stores only [code] (into `cardio_entry.type`); [resolve] turns a stored code back into
 * the right activity, consulting the user's custom definitions first for a `custom_` code.
 */
sealed interface CardioActivity {
    val code: String
    val displayName: String
    val icon: ImageVector
    /** A recovery day — clears distance/effort and is excluded from weekly totals. Only [CardioType.REST]. */
    val isRest: Boolean
    /** Surfaces the interval-count field. Only [CardioType.HIIT]. */
    val isHiit: Boolean

    data class Builtin(val type: CardioType) : CardioActivity {
        override val code: String get() = type.code
        override val displayName: String get() = type.displayName
        override val icon: ImageVector get() = type.icon
        override val isRest: Boolean get() = type.isRest
        override val isHiit: Boolean get() = type == CardioType.HIIT
    }

    data class Custom(val custom: CustomCardioType) : CardioActivity {
        override val code: String get() = custom.code
        override val displayName: String get() = custom.name
        override val icon: ImageVector get() = CardioGlyphs.icon(custom.glyphKey)
        // Custom activities are always plain steady-state work — no rest/HIIT special-casing.
        override val isRest: Boolean get() = false
        override val isHiit: Boolean get() = false
    }

    companion object {
        val RUN: CardioActivity = Builtin(CardioType.RUN)

        /**
         * Resolve a stored `cardio_entry.type` to an activity. A `custom_` code looks up [customs]
         * (falling back to [CardioType.OTHER] if that definition was since deleted, so an orphaned
         * session still renders as "Other" rather than a raw code); any other code goes through the
         * built-in enum (which itself maps unknowns to OTHER).
         */
        fun resolve(code: String, customs: List<CustomCardioType>): CardioActivity =
            if (CustomCardioType.isCustomCode(code)) {
                customs.firstOrNull { it.code == code }?.let { Custom(it) } ?: Builtin(CardioType.OTHER)
            } else {
                Builtin(CardioType.fromCode(code))
            }
    }
}
