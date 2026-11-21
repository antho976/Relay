package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * A training block (Coach v3 C) — a few weeks with ONE intent, persisted so the coach can plan
 * across weeks instead of reacting inside them.
 *
 * V2's "mesocycle" was a copy string that changed no behavior. This row is the real thing: the
 * weekly pass advances it, the advisors consult it, and deloads become scheduled and announced
 * rather than only ever emergency responses to accumulated fatigue.
 *
 * [phase] is a [com.forge.app.domain.coach.BlockPhase] code. [weekIndex] is 1-based (week 1 of N).
 */
@Entity(tableName = "training_block")
data class TrainingBlock(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** ACCUMULATE | INTENSIFY | PEAK | DELOAD, as a durable code. */
    @ColumnInfo(name = "phase") val phase: String,
    /** Which week of the block this is, 1-based. */
    @ColumnInfo(name = "week_index") val weekIndex: Int,
    /** How many weeks the block runs before its deload. */
    @ColumnInfo(name = "planned_weeks") val plannedWeeks: Int,
    /** The goal this block serves — a `coach_goal` id, or 0 when it's general training. */
    @ColumnInfo(name = "focus_goal_id") val focusGoalId: Long = 0,
    /** The coach's one-line statement of what this block is for. */
    @ColumnInfo(name = "intent") val intent: String = "",
    @ColumnInfo(name = "started_at") val startedAt: Long,
    /** ISO week id the block last advanced on — makes advancing idempotent per week. */
    @ColumnInfo(name = "last_advanced_week") val lastAdvancedWeek: String = "",
    /** When the block finished; null while it's the live one. */
    @ColumnInfo(name = "ended_at") val endedAt: Long? = null
) {
    val isActive: Boolean get() = endedAt == null
}
