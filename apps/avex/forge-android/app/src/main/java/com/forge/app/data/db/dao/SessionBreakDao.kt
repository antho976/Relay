package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import com.forge.app.data.db.entities.SessionBreak
import kotlinx.coroutines.flow.Flow

@Dao
interface SessionBreakDao {
    @Insert
    suspend fun insert(brk: SessionBreak): Long

    @Query("SELECT * FROM session_break WHERE session_id = :sessionId ORDER BY logged_at ASC")
    fun observeForSession(sessionId: Long): Flow<List<SessionBreak>>

    @Query("SELECT COUNT(*) FROM session_break WHERE session_id = :sessionId")
    suspend fun countForSession(sessionId: Long): Int
}
