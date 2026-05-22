package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey
import com.forge.app.data.db.types.EffortRating

/**
 * One exercise slot within a session. `exerciseId` references the static
 * [com.forge.app.program.ExercisePlan.id] (e.g. "ua1", "lb6") — no FK to a
 * Room table because the catalogue lives in code, not the DB.
 *
 * `swappedName` / `swappedUnit` are set when the user uses the swap feature
 * during this specific session — they override the static plan for this entry.
 * They are NOT the same as [ExerciseCustomization] (which is a persistent swap
 * carried across all future sessions for the same `exerciseId`).
 *
 * Cascade delete: deleting the parent Session removes its LoggedExercises and,
 * transitively, their LoggedSets.
 */
@Entity(
    tableName = "logged_exercise",
    foreignKeys = [
        ForeignKey(
            entity = Session::class,
            parentColumns = ["id"],
            childColumns = ["session_id"],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index("session_id")]
)
data class LoggedExercise(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "session_id") val sessionId: Long,
    @ColumnInfo(name = "exercise_id") val exerciseId: String,
    @ColumnInfo(name = "order_index") val orderIndex: Int,
    @ColumnInfo(name = "swapped_name") val swappedName: String? = null,
    @ColumnInfo(name = "swapped_unit") val swappedUnit: String? = null,
    @ColumnInfo(name = "difficulty") val difficulty: EffortRating? = null,
    @ColumnInfo(name = "hit_full_target") val hitFullTarget: Boolean = false,
    @ColumnInfo(name = "was_pr") val wasPr: Boolean = false,
    @ColumnInfo(name = "note") val note: String? = null,
    @ColumnInfo(name = "skipped") val skipped: Boolean = false
)
