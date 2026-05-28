package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * One performed set within a logged exercise.
 *
 * Both `weightText` and `weightLb` are stored:
 *  - `weightText` is what the user typed verbatim ("BW", "2 plates", "45")
 *    and is what's shown back in the UI.
 *  - `weightLb` is the parsed numeric value used by aggregates (volume,
 *    PR detection, strength curves). `null` when the input is non-numeric
 *    (e.g. "BW") — aggregates should treat that as 0 lb or skip it.
 */
@Entity(
    tableName = "logged_set",
    foreignKeys = [
        ForeignKey(
            entity = LoggedExercise::class,
            parentColumns = ["id"],
            childColumns = ["logged_exercise_id"],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index("logged_exercise_id")]
)
data class LoggedSet(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "logged_exercise_id") val loggedExerciseId: Long,
    @ColumnInfo(name = "set_index") val setIndex: Int,
    @ColumnInfo(name = "weight_text") val weightText: String,
    @ColumnInfo(name = "weight_lb") val weightLb: Double? = null,
    @ColumnInfo(name = "reps") val reps: Int,
    @ColumnInfo(name = "completed_at") val completedAt: Long,
    /** Per-set difficulty tag (#68): "easy" | "hard" | null (no tag). */
    @ColumnInfo(name = "difficulty_tag") val difficultyTag: String? = null,
    /** AMRAP marker (#140): reps field = achieved count; intent was max reps. */
    @ColumnInfo(name = "is_amrap") val isAmrap: Boolean = false,
    /** Assisted set (#141): bands / spotter — excluded from all-time PR comparison. */
    @ColumnInfo(name = "is_assisted") val isAssisted: Boolean = false,
    /** Failure marker (#18): set ended at muscular failure. */
    @ColumnInfo(name = "to_failure") val toFailure: Boolean = false,
    /**
     * Advanced set type (#142): null = normal | "drop" | "myo" | "rest_pause"
     * | "negative" | "paused" | "partial" | "isometric" | "cluster" | "emom"
     */
    @ColumnInfo(name = "set_type") val setType: String? = null,
    /** Mid-set weight drop annotation (#143): "weightLb2/reps2" e.g. "35/4". */
    @ColumnInfo(name = "drop_annotation") val dropAnnotation: String? = null,
    /** Per-set RPE (Rate of Perceived Exertion), 1.0–10.0 in 0.5 steps. */
    @ColumnInfo(name = "rpe") val rpe: Double? = null
)
