package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * A placed exercise within a [ProgramDay] (program-unlock plan, Phase 0): which library movement,
 * in what order, with the day-specific sets/reps. [exerciseLibId] references an `ExerciseLibrary`
 * entry (authored in code), not a DB row. Indexed by [dayId] for per-day reads.
 */
@Entity(
    tableName = "program_slot",
    indices = [Index("day_id")]
)
data class ProgramSlot(
    @PrimaryKey @ColumnInfo(name = "id") val id: String,
    @ColumnInfo(name = "day_id") val dayId: String,
    @ColumnInfo(name = "position") val position: Int,
    @ColumnInfo(name = "exercise_lib_id") val exerciseLibId: String,
    @ColumnInfo(name = "sets") val sets: Int,
    @ColumnInfo(name = "reps") val reps: String
)
