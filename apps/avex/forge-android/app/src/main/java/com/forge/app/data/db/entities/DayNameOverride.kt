package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * User-supplied label for a day key. When absent, the UI falls back to
 * [com.forge.app.program.DayPlan.defaultName]. Cleared via DELETE to revert.
 */
@Entity(tableName = "day_name_override")
data class DayNameOverride(
    @PrimaryKey @ColumnInfo(name = "day_key") val dayKey: String,
    @ColumnInfo(name = "custom_name") val customName: String
)
