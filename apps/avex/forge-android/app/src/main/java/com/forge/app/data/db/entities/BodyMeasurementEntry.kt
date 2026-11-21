package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * A single body-measurement reading (GYMAP-52) — one circumference (waist/chest/arms/thighs/hips)
 * on one day. One row per (type, day), enforced by the unique (type, date_key) index so REPLACE
 * upserts today's value instead of appending. Mirrors [BodyweightEntry]; value stored canonically
 * in cm regardless of the user's display-unit preference.
 */
@Entity(
    tableName = "body_measurement",
    indices = [Index(value = ["type", "date_key"], unique = true)]
)
data class BodyMeasurementEntry(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** Stable measurement discriminator — [com.forge.app.domain.measurement.BodyMeasurementType.key]. */
    @ColumnInfo(name = "type") val type: String,
    /** ISO date string "yyyy-MM-dd" — one entry per type per day (upserted by date). */
    @ColumnInfo(name = "date_key") val dateKey: String,
    /** Circumference in cm (converted from inches at input if needed). */
    @ColumnInfo(name = "value_cm") val valueCm: Double,
    @ColumnInfo(name = "recorded_at") val recordedAt: Long
)
