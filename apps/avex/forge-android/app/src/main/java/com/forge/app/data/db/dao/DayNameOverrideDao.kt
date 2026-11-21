package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Query
import com.forge.app.data.db.entities.DayNameOverride
import kotlinx.coroutines.flow.Flow

@Dao
interface DayNameOverrideDao {

    @Query("SELECT * FROM day_name_override WHERE day_key = :dayKey")
    suspend fun get(dayKey: String): DayNameOverride?

    @Query("SELECT * FROM day_name_override")
    fun observeAll(): Flow<List<DayNameOverride>>
}
