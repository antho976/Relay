package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import com.forge.app.data.db.entities.RestEvent

@Dao
interface RestEventDao {

    @Insert
    suspend fun insert(event: RestEvent): Long

    /** One session's rest events, oldest first. */
    @Query("SELECT * FROM rest_event WHERE session_id = :sessionId ORDER BY logged_at")
    suspend fun forSession(sessionId: Long): List<com.forge.app.data.db.entities.RestEvent>

    /** Most recent rest events — RestAdvisor tuning input; old behavior ages out naturally. */
    @Query("SELECT * FROM rest_event ORDER BY logged_at DESC LIMIT :limit")
    suspend fun recent(limit: Int = 200): List<RestEvent>

    @Query("DELETE FROM rest_event")
    suspend fun deleteAll()
}
