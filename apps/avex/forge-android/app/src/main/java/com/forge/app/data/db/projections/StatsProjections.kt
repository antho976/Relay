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
    /** Stable id of the owning LoggedExercise — lets PR display match the exact set (not just session+exercise). */
    @ColumnInfo(name = "logged_exercise_id") val loggedExerciseId: Long,
    @ColumnInfo(name = "rpe") val rpe: Double? = null,
    /** The logged exercise's stored name; for an imported lift the catalogue can't match (`ext-*`) it is the only readable label. */
    @ColumnInfo(name = "swapped_name") val swappedName: String? = null
)

/**
 * One tracked, non-skipped set for the Stats page, with [isStrengthSet] false for a timed hold or an
 * assisted set. A single whole-history query feeds both populations: the strength one (e1RM, PRs)
 * keeps only strength sets, and the activity one (consistency, sets per muscle) keeps every row with
 * the weight nulled on the others.
 */
data class StatsSetRow(
    @androidx.room.Embedded val set: SetWithExerciseAndSession,
    @ColumnInfo(name = "is_strength_set") val isStrengthSet: Boolean
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
