package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * A single item in a user-defined warmup routine for a day (#144).
 * Replaces the simpler DataStore-based checklist from item #120.
 */
@Entity(tableName = "warmup_routine_item")
data class WarmupRoutineItem(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "day_key") val dayKey: String,
    @ColumnInfo(name = "label") val label: String,
    @ColumnInfo(name = "order_index") val orderIndex: Int,
    /** Optional duration hint shown to the user (e.g. "30s"). */
    @ColumnInfo(name = "duration_hint") val durationHint: String? = null
)
