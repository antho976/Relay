package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Query
import com.forge.app.data.db.entities.WarmupRoutineItem
import kotlinx.coroutines.flow.Flow

@Dao
interface WarmupRoutineDao {

    @Query("SELECT * FROM warmup_routine_item WHERE day_key = :dayKey ORDER BY order_index ASC")
    fun observeForDay(dayKey: String): Flow<List<WarmupRoutineItem>>

    @Query("SELECT * FROM warmup_routine_item WHERE day_key = :dayKey ORDER BY order_index ASC")
    suspend fun forDay(dayKey: String): List<WarmupRoutineItem>

    @Query("DELETE FROM warmup_routine_item")
    suspend fun deleteAll()
}
