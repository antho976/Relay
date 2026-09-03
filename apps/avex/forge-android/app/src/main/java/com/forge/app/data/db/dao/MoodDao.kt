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

    /** One row per session for a BATCH of sessions (P-01). See [LoggedExerciseDao.forSessions]. */
    @Query("SELECT * FROM mood_entry WHERE session_id IN (:sessionIds)")
    suspend fun forSessions(sessionIds: List<Long>): List<MoodEntry>

    @Query("SELECT * FROM mood_entry")
    fun observeAll(): Flow<List<MoodEntry>>

    /** Entries since [sinceMs], newest first — adaptation-engine recovery/readiness input. */
    @Query("SELECT * FROM mood_entry WHERE recorded_at >= :sinceMs ORDER BY recorded_at DESC")
    suspend fun since(sinceMs: Long): List<MoodEntry>

    /** Wipe every mood entry. Deleting a session only SETs its mood's `session_id` NULL (deliberate,
     *  see [com.forge.app.data.db.entities.MoodEntry]) — so "reset session data" has to clear these
     *  explicitly or orphaned moods keep feeding readiness for workouts that no longer exist. */
    @Query("DELETE FROM mood_entry")
    suspend fun deleteAll()
}
