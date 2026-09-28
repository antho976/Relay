package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Query

/**
 * Nothing writes `rest_day_entry` any more (its repository was retired); the session reset still
 * clears rows an older install or a restored backup left behind.
 */
@Dao
interface RestDayDao {

    @Query("DELETE FROM rest_day_entry")
    suspend fun deleteAll()
}
