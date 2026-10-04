package com.tally.app.data.db

import androidx.room.TypeConverter
import java.time.LocalDate

/** Dates are stored as epoch days: sortable, range-queryable and timezone-free. */
class Converters {
    @TypeConverter fun fromDate(date: LocalDate?): Long? = date?.toEpochDay()
    @TypeConverter fun toDate(epochDay: Long?): LocalDate? = epochDay?.let(LocalDate::ofEpochDay)
}
