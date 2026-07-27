package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * Daily lean-body-mass reading (W6) — skeletal muscle from a watch's BIA measurement, imported
 * from Health Connect. One row per day via the unique date_key index, the exact shape of
 * [BodyFatEntry]. Import-only: Avex never measures or writes lean mass, a watch does — so there
 * is no manual-log path and no write-back.
 */
@Entity(
    tableName = "lean_mass",
    indices = [Index(value = ["date_key"], unique = true)]
)
data class LeanMassEntry(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** ISO date string "yyyy-MM-dd" — one entry per day (upserted by date). */
    @ColumnInfo(name = "date_key") val dateKey: String,
    /** Lean body mass in pounds (app storage convention; rendered via the weight-unit setting). */
    @ColumnInfo(name = "weight_lb") val weightLb: Double,
    @ColumnInfo(name = "recorded_at") val recordedAt: Long
)
