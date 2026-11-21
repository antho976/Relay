package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/** A holiday or vacation range — streak and deload counters pause during this window (#135). */
@Entity(tableName = "vacation_period")
data class VacationPeriod(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** Inclusive start date key "yyyy-MM-dd". */
    @ColumnInfo(name = "start_date") val startDate: String,
    /** Inclusive end date key "yyyy-MM-dd". */
    @ColumnInfo(name = "end_date") val endDate: String,
    @ColumnInfo(name = "label") val label: String = ""
)
