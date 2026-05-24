package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.TrophyNearMiss
import kotlinx.coroutines.flow.Flow

@Dao
interface TrophyNearMissDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(entry: TrophyNearMiss): Long

    @Query("SELECT * FROM trophy_near_miss ORDER BY recorded_at DESC LIMIT 50")
    fun observeRecent(): Flow<List<TrophyNearMiss>>

    @Query("DELETE FROM trophy_near_miss")
    suspend fun deleteAll()
}
