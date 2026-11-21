package com.forge.app.data.db

import androidx.room.TypeConverter
import com.forge.app.data.db.types.EffortRating

/**
 * Room TypeConverters. Each enum stored in the DB is converted via its [EffortRating.code]
 * string (or equivalent), not its ordinal — survives enum reordering and is human-readable
 * via the SQLite browser.
 */
class Converters {

    @TypeConverter
    fun effortRatingToString(rating: EffortRating?): String? = rating?.code

    @TypeConverter
    fun stringToEffortRating(code: String?): EffortRating? =
        code?.let { EffortRating.fromCode(it) }
}
