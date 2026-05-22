package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.DayNameOverride
import kotlinx.coroutines.flow.Flow

@Dao
interface DayNameOverrideDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(override: DayNameOverride)

    @Query("DELETE FROM day_name_override WHERE day_key = :dayKey")
    suspend fun clear(dayKey: String)

    @Query("SELECT * FROM day_name_override WHERE day_key = :dayKey")
    suspend fun get(dayKey: String): DayNameOverride?

    @Query("SELECT * FROM day_name_override")
    fun observeAll(): Flow<List<DayNameOverride>>
}
