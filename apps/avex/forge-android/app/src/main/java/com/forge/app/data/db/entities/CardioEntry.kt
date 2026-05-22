package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * A manually-logged cardio session. Standalone — not joined to [Session].
 *
 * `type`, `effort`, and `restReason` are stored as plain strings rather than enums.
 * The Phase 7 UI defines the allowed values; storing as strings keeps the schema
 * flexible while the cardio feature is still being shaped.
 *
 * Type values used by the prototype: "run" | "walk" | "treadmill" | "rest" | "other"
 * Effort:                            "easy" | "moderate" | "hard"
 * Rest reason:                       "planned" | "sore" | "sick" | "busy"
 */
@Entity(tableName = "cardio_entry")
data class CardioEntry(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "date") val date: Long,
    @ColumnInfo(name = "type") val type: String,
    @ColumnInfo(name = "duration_min") val durationMin: Int,
    @ColumnInfo(name = "distance_km") val distanceKm: Double? = null,
    @ColumnInfo(name = "effort") val effort: String? = null,
    @ColumnInfo(name = "rest_reason") val restReason: String? = null,
    @ColumnInfo(name = "note") val note: String? = null
)
