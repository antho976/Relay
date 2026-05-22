package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * Persistent per-exercise swap. When present, the day view shows this swap by
 * default in place of the static [com.forge.app.program.ExercisePlan] for the
 * matching `exerciseId`. Cleared via DELETE to revert.
 *
 * Independent of [LoggedExercise.swappedName] — that one is the swap chosen
 * *for one specific session entry*; this one carries forward to future sessions.
 */
@Entity(tableName = "exercise_customization")
data class ExerciseCustomization(
    @PrimaryKey @ColumnInfo(name = "exercise_id") val exerciseId: String,
    @ColumnInfo(name = "swapped_name") val swappedName: String,
    @ColumnInfo(name = "swapped_unit") val swappedUnit: String
)
