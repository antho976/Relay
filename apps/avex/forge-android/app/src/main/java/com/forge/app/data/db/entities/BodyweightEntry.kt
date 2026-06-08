package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

/** Daily bodyweight log entry (#76). One row per day, enforced by the unique date_key index. */
@Entity(
    tableName = "bodyweight_entry",
    indices = [Index(value = ["date_key"], unique = true)]
)
data class BodyweightEntry(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** ISO date string "yyyy-MM-dd" — one entry per day (upserted by date). */
    @ColumnInfo(name = "date_key") val dateKey: String,
    /** Weight in lb (converted from kg at input if needed). */
    @ColumnInfo(name = "weight_lb") val weightLb: Double,
    @ColumnInfo(name = "recorded_at") val recordedAt: Long
)
