package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.SessionHrSample

@Dao
interface SessionHrSampleDao {

    /** Batches can overlap after a BT retry — the (session, at_ms) key makes re-insert idempotent. */
    @Insert(onConflict = OnConflictStrategy.IGNORE)
    suspend fun insertAll(samples: List<SessionHrSample>)

    @Query("SELECT * FROM session_hr_sample WHERE session_id = :sessionId ORDER BY at_ms")
    suspend fun forSession(sessionId: Long): List<SessionHrSample>

    @Query("SELECT COUNT(*) FROM session_hr_sample WHERE session_id = :sessionId")
    suspend fun countForSession(sessionId: Long): Int

    @Query("DELETE FROM session_hr_sample WHERE session_id = :sessionId")
    suspend fun deleteForSession(sessionId: Long)
}
