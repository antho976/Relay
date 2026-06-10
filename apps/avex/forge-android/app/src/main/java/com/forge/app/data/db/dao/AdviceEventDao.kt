package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import com.forge.app.data.db.entities.AdviceEvent

@Dao
interface AdviceEventDao {

    @Insert
    suspend fun insert(event: AdviceEvent): Long

    /** Full history for one recommendation id — cooldown checks ("dismissed twice → mute"). */
    @Query("SELECT * FROM advice_event WHERE advice_id = :adviceId ORDER BY logged_at DESC")
    suspend fun forAdvice(adviceId: String): List<AdviceEvent>

    @Query("SELECT * FROM advice_event ORDER BY logged_at DESC LIMIT :limit")
    suspend fun recent(limit: Int = 500): List<AdviceEvent>

    @Query("DELETE FROM advice_event")
    suspend fun deleteAll()
}
