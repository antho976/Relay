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

    /** Drop near-miss rows for the given trophies — used to prune ones that have since unlocked,
     *  so an already-earned trophy can't keep showing up as a near-miss / "Up next" nudge. */
    @Query("DELETE FROM trophy_near_miss WHERE trophy_id IN (:trophyIds)")
    suspend fun deleteForTrophies(trophyIds: Collection<String>)

    @Query("DELETE FROM trophy_near_miss")
    suspend fun deleteAll()
}
