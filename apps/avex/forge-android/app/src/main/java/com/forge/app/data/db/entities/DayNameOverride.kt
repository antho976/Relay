package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * Legacy user-supplied label for a day key, from before the plan builder renamed days itself.
 * Nothing writes it any more: migration 39→40 folded every row into `program_day.name` and emptied
 * the table, so a day's name is the program's. Kept as a table for the schema history.
 */
@Entity(tableName = "day_name_override")
data class DayNameOverride(
    @PrimaryKey @ColumnInfo(name = "day_key") val dayKey: String,
    @ColumnInfo(name = "custom_name") val customName: String
)
