package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import com.forge.app.data.db.entities.MoodEntry
import kotlinx.coroutines.flow.Flow

@Dao
interface MoodDao {

    @Insert
    suspend fun insert(entry: MoodEntry): Long

    @Query("SELECT * FROM mood_entry ORDER BY recorded_at DESC LIMIT :limit")
    fun observeRecent(limit: Int = 20): Flow<List<MoodEntry>>

    @Query("SELECT * FROM mood_entry WHERE session_id = :sessionId LIMIT 1")
    suspend fun forSession(sessionId: Long): MoodEntry?

    @Query("SELECT * FROM mood_entry")
    fun observeAll(): Flow<List<MoodEntry>>
}
