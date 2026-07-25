package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * One objective the coach is working toward (Coach v3 A2) — the Goal Portfolio's row type.
 *
 * Deliberately ADDITIVE to the older goal tables (`exercise_goal`, `extended_goal`): those have live
 * readers (the Goals screen, the cardio hub, and the `goal_crusher` / `goals_5` trophies via
 * `UnlockRule.ExerciseGoalsAchievedAtLeast`), so nothing is migrated or rewritten. The portfolio
 * READS them as candidates and offers a one-tap promotion into a coach goal; a user who never
 * promotes anything keeps exactly the app they had.
 *
 * [kind] is a [com.forge.app.domain.coach.CoachGoalKind] code, stored as a string for the same
 * reason every other enum here is (a code stays interpretable forever). [targetKey] scopes the
 * goal — an exercise id, a muscle code, a ratio name — and is empty for whole-athlete goals.
 */
@Entity(tableName = "coach_goal", indices = [Index("kind")])
data class CoachGoal(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** [com.forge.app.domain.coach.CoachGoalKind.code]. */
    @ColumnInfo(name = "kind") val kind: String,
    /** Exercise id / muscle code / ratio name this goal is about; empty for whole-athlete goals. */
    @ColumnInfo(name = "target_key") val targetKey: String = "",
    /** The number being chased, in the metric's own unit (lb, sets/week, sessions/week, minutes). */
    @ColumnInfo(name = "target_value") val targetValue: Double? = null,
    /** 0 = the block's focus goal; everything else gets a maintenance floor. Lower sorts first. */
    @ColumnInfo(name = "priority") val priority: Int = 0,
    @ColumnInfo(name = "created_at") val createdAt: Long,
    /** When the target was reached — the lifecycle moment that earns a celebration + a successor. */
    @ColumnInfo(name = "completed_at") val completedAt: Long? = null,
    /** Retired without being reached (or after being celebrated); keeps the ledger honest. */
    @ColumnInfo(name = "archived_at") val archivedAt: Long? = null,
    /** "user" | "coach" — a coach-proposed goal the user accepted still records who thought of it. */
    @ColumnInfo(name = "source") val source: String = "user",
    /** Free text the user attached; never generated. */
    @ColumnInfo(name = "note") val note: String = ""
) {
    /** Active = not completed, not archived. The portfolio only reasons about active goals. */
    val isActive: Boolean get() = completedAt == null && archivedAt == null
}
