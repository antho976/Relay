package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.Query
import androidx.room.Update
import com.forge.app.data.db.entities.Session
import kotlinx.coroutines.flow.Flow

@Dao
interface SessionDao {

    @Insert
    suspend fun insert(session: Session): Long

    @Update
    suspend fun update(session: Session)

    @Delete
    suspend fun delete(session: Session)

    @Query("SELECT * FROM session WHERE id = :id")
    suspend fun get(id: Long): Session?

    /** Returns the in-progress session if one exists. App logic limits this to at most one. */
    @Query("SELECT * FROM session WHERE finished_at IS NULL ORDER BY started_at DESC LIMIT 1")
    suspend fun getActiveSession(): Session?

    @Query("SELECT * FROM session WHERE finished_at IS NULL ORDER BY started_at DESC LIMIT 1")
    fun observeActiveSession(): Flow<Session?>

    @Query("SELECT * FROM session WHERE finished_at IS NOT NULL ORDER BY finished_at DESC LIMIT :limit")
    fun observeRecent(limit: Int = 10): Flow<List<Session>>

    @Query("""
        SELECT * FROM session
        WHERE day_key = :dayKey AND finished_at IS NOT NULL
        ORDER BY finished_at DESC LIMIT 1
    """)
    suspend fun lastFinishedForDay(dayKey: String): Session?

    /** Used by the "Showing Up" / "Through the Door" trophy rules. */
    @Query("SELECT COUNT(*) FROM session WHERE finished_at IS NOT NULL")
    fun observeFinishedCount(): Flow<Int>

    /** Used by the "Full Week" trophy (all 4 days trained). */
    @Query("SELECT DISTINCT day_key FROM session WHERE finished_at IS NOT NULL")
    suspend fun distinctDayKeysTrained(): List<String>

    /** For the deload banner — sessions completed since the user last marked deload. */
    @Query("SELECT COUNT(*) FROM session WHERE finished_at IS NOT NULL")
    suspend fun finishedCount(): Int

    /** Rolling-window queries for the Overview weekly stats strip. */
    @Query("SELECT COUNT(*) FROM session WHERE finished_at IS NOT NULL AND finished_at >= :sinceEpochMs")
    fun observeFinishedCountSince(sinceEpochMs: Long): Flow<Int>

    @Query("SELECT SUM(total_volume_lb) FROM session WHERE finished_at IS NOT NULL AND finished_at >= :sinceEpochMs")
    fun observeVolumeSince(sinceEpochMs: Long): Flow<Double?>
}
