package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * User-set target weight for an exercise. One row per exerciseId.
 * Conflict policy on insert: REPLACE (upsert semantics — updating a goal overwrites the old one).
 */
@Entity(tableName = "exercise_goal")
data class ExerciseGoal(
    @PrimaryKey
    @ColumnInfo(name = "exercise_id") val exerciseId: String,
    @ColumnInfo(name = "target_weight_lb") val targetWeightLb: Double,
    @ColumnInfo(name = "created_at") val createdAt: Long
)
