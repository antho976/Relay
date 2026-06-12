package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import com.forge.app.data.db.entities.SessionSegment

@Dao
interface SessionSegmentDao {

    @Insert
    suspend fun insert(segment: SessionSegment): Long

    @Query("SELECT * FROM session_segment WHERE session_id = :sessionId AND ended_at IS NULL")
    suspend fun openForSession(sessionId: Long): List<SessionSegment>

    @Query("SELECT * FROM session_segment WHERE session_id = :sessionId ORDER BY started_at")
    suspend fun forSession(sessionId: Long): List<SessionSegment>

    /** Close every still-open segment of a session (explicit leave / finish). */
    @Query("UPDATE session_segment SET ended_at = :endedAt WHERE session_id = :sessionId AND ended_at IS NULL")
    suspend fun closeOpen(sessionId: Long, endedAt: Long)

    /** Close one segment at a best-effort time (dangling-segment cleanup after process death). */
    @Query("UPDATE session_segment SET ended_at = :endedAt WHERE id = :id")
    suspend fun close(id: Long, endedAt: Long)
}
