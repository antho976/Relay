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
 * HR zone:                           "1".."5" (manual training-zone tag), or null
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
    @ColumnInfo(name = "note") val note: String? = null,
    /** Number of intervals for a HIIT / interval session; null for steady-state work (DB v23). */
    @ColumnInfo(name = "interval_count") val intervalCount: Int? = null,
    /** Manually-logged HR training zone ("1".."5"), or null when not tracked (DB v23). */
    @ColumnInfo(name = "hr_zone") val hrZone: String? = null,
    /** Treadmill / elliptical incline as a percent grade; null for activities without one (DB v27, GYMAP-38). */
    @ColumnInfo(name = "incline_pct") val inclinePct: Double? = null,
    /** Pool lengths (laps) for a swim; null otherwise (DB v27, GYMAP-38). */
    @ColumnInfo(name = "laps") val laps: Int? = null,
    /** Elevation gain in metres for outdoor distance work (run/walk/hike/cycle); null otherwise (DB v27, GYMAP-38). */
    @ColumnInfo(name = "elevation_m") val elevationM: Double? = null,
    /** Weather / environment tags as comma-joined codes (hot/cold/rain/wind); null when none (DB v28, GYMAP-39).
     *  Decoded via [com.forge.app.domain.cardio.CardioCondition.decode]. */
    @ColumnInfo(name = "conditions") val conditions: String? = null
)
