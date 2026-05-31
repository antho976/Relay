package com.forge.app.data.db.projections

import androidx.room.ColumnInfo

/**
 * SELECT projections for the gym stats subtab. Not @Entity — these are typed
 * containers for join results. Aliased column names match what the underlying
 * queries emit.
 */
data class SetWithExerciseId(
    @ColumnInfo(name = "weight_lb") val weightLb: Double?,
    @ColumnInfo(name = "reps") val reps: Int,
    @ColumnInfo(name = "exercise_id") val exerciseId: String
)

data class SetWithExerciseAndSession(
    @ColumnInfo(name = "weight_lb") val weightLb: Double?,
    @ColumnInfo(name = "reps") val reps: Int,
    @ColumnInfo(name = "exercise_id") val exerciseId: String,
    @ColumnInfo(name = "started_at") val sessionStartedAt: Long,
    @ColumnInfo(name = "rpe") val rpe: Double? = null
)

/**
 * One row per session in the heatmap window — used to count exercises per day after
 * grouping in Kotlin. The query returns `started_at` for each LoggedExercise; multiple
 * rows for the same session are fine because we aggregate to a single per-day count.
 */
data class HeatmapTimestamp(
    @ColumnInfo(name = "started_at") val startedAt: Long
)

data class RecentPrRow(
    @ColumnInfo(name = "exercise_id") val exerciseId: String,
    @ColumnInfo(name = "swapped_name") val swappedName: String?,
    @ColumnInfo(name = "started_at") val sessionStartedAt: Long,
    @ColumnInfo(name = "logged_exercise_id") val loggedExerciseId: Long
)

/**
 * Per-session aggregate for one exercise: feeds the day-screen last-session strip + sparkline.
 * Volume = SUM(weight_lb * reps) restricted to this exercise within that session.
 * Top weight = MAX(weight_lb) for that exercise in that session.
 */
data class ExerciseSessionAggregate(
    @ColumnInfo(name = "started_at") val sessionStartedAt: Long,
    @ColumnInfo(name = "finished_at") val sessionFinishedAt: Long?,
    @ColumnInfo(name = "volume_lb") val volumeLb: Double,
    @ColumnInfo(name = "top_weight_lb") val topWeightLb: Double?
)
