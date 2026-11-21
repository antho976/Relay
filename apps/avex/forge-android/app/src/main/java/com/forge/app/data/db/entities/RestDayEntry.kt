package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * Marks an intentional rest day on the calendar (#114 #115).
 * [type]: "planned" | "sick" | "travel"
 * [dateKey]: "yyyy-MM-dd" string for the calendar day.
 */
@Entity(tableName = "rest_day_entry")
data class RestDayEntry(
    @PrimaryKey
    @ColumnInfo(name = "date_key") val dateKey: String,
    @ColumnInfo(name = "type") val type: String = "planned",
    @ColumnInfo(name = "note") val note: String = ""
)
