package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * Daily body-fat-percentage log entry (GYMAP-62). One row per day, enforced by the unique date_key
 * index. Kept as its own table (not a column on [BodyweightEntry]) because Health Connect stores
 * body fat as a separate record with its own timestamp — a scale can write weight and body fat at
 * different instants, and a reading can arrive with no matching weigh-in.
 */
@Entity(
    tableName = "body_fat",
    indices = [Index(value = ["date_key"], unique = true)]
)
data class BodyFatEntry(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** ISO date string "yyyy-MM-dd" — one entry per day (upserted by date). */
    @ColumnInfo(name = "date_key") val dateKey: String,
    /** Body fat as a percentage (0–100), stored to one decimal of resolution. */
    @ColumnInfo(name = "percent") val percent: Double,
    @ColumnInfo(name = "recorded_at") val recordedAt: Long
)
