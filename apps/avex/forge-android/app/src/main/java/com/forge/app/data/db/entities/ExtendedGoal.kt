package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * Extended goal system (#137). Supports multiple goal types beyond simple weight targets.
 * Goal types: "1rm" | "weekly_volume" | "frequency" | "monthly_prs"
 * Stretch goal: same types but with a harder target shown alongside the main goal.
 */
@Entity(tableName = "extended_goal")
data class ExtendedGoal(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** Nullable — global goals (e.g. monthly PRs) have no exercise. */
    @ColumnInfo(name = "exercise_id") val exerciseId: String? = null,
    /** "1rm" | "weekly_volume" | "frequency" | "monthly_prs" */
    @ColumnInfo(name = "goal_type") val goalType: String,
    @ColumnInfo(name = "target_value") val targetValue: Double,
    /** Optional harder target displayed alongside main. */
    @ColumnInfo(name = "stretch_value") val stretchValue: Double? = null,
    @ColumnInfo(name = "label") val label: String = "",
    @ColumnInfo(name = "created_at") val createdAt: Long,
    /** When this goal was completed (null = not yet). */
    @ColumnInfo(name = "completed_at") val completedAt: Long? = null
)
