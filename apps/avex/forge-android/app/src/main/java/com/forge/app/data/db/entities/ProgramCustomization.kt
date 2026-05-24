package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * Persisted program customizations — overlays applied on top of the static program (#90, #91, #92).
 * One row per exercise per day. The exercise can be:
 *   - from the static program (exerciseId matches a plan id like "ua1")
 *   - added by the user (exerciseId is "custom_<uuid>", customName is non-null)
 *
 * If [removed] is true, the exercise is hidden from the day.
 * [repRangeOverride] replaces the plan's reps string (e.g. "6-8" instead of "8-10").
 * [setsOverride] overrides the planned set count (0 = use plan default).
 * [orderOverride] drives the display order within the day (lower = earlier).
 */
@Entity(tableName = "program_customization", primaryKeys = ["day_key", "exercise_id"])
data class ProgramCustomization(
    @ColumnInfo(name = "day_key") val dayKey: String,
    @ColumnInfo(name = "exercise_id") val exerciseId: String,
    @ColumnInfo(name = "custom_name") val customName: String? = null,
    @ColumnInfo(name = "custom_muscle") val customMuscle: String? = null,
    @ColumnInfo(name = "rep_range_override") val repRangeOverride: String? = null,
    @ColumnInfo(name = "sets_override") val setsOverride: Int = 0,
    @ColumnInfo(name = "order_override") val orderOverride: Int = 999,
    @ColumnInfo(name = "removed") val removed: Boolean = false
)
