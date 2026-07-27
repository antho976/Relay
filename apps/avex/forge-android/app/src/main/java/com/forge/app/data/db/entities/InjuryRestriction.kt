package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * "Don't give me this until it's better" (Coach v3 B1) — a persistent restriction on a muscle or a
 * movement, distinct from acute soreness.
 *
 * Soreness fades in days and is read from the check-in; an injury lasts weeks and needs an explicit
 * end. Conflating them is how a coach ends up either ignoring a tweaked shoulder or treating normal
 * DOMS as an injury. The generator, the directive and the session adaptor all route around an
 * active restriction.
 *
 * [clearedAt] null = still active. Restrictions are never deleted on clearing: "I hurt my shoulder
 * for three weeks in July" is exactly the context that explains that month's numbers later.
 */
@Entity(tableName = "injury_restriction")
data class InjuryRestriction(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** "muscle" | "exercise" — what [targetKey] names. */
    @ColumnInfo(name = "scope") val scope: String,
    /** A [com.forge.app.program.MuscleGroup] code, or an exercise library id. */
    @ColumnInfo(name = "target_key") val targetKey: String,
    /** The user's own words; never generated. */
    @ColumnInfo(name = "note") val note: String = "",
    @ColumnInfo(name = "started_at") val startedAt: Long,
    /** When it was cleared, or null while it still applies. */
    @ColumnInfo(name = "cleared_at") val clearedAt: Long? = null
) {
    val isActive: Boolean get() = clearedAt == null

    companion object {
        const val SCOPE_MUSCLE = "muscle"
        const val SCOPE_EXERCISE = "exercise"
    }
}
