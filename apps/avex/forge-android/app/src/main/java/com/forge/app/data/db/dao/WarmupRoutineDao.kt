package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.forge.app.data.db.entities.WarmupRoutineItem
import kotlinx.coroutines.flow.Flow

@Dao
interface WarmupRoutineDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(item: WarmupRoutineItem): Long

    @Update
    suspend fun update(item: WarmupRoutineItem)

    @Delete
    suspend fun delete(item: WarmupRoutineItem)

    @Query("SELECT * FROM warmup_routine_item WHERE day_key = :dayKey ORDER BY order_index ASC")
    fun observeForDay(dayKey: String): Flow<List<WarmupRoutineItem>>

    @Query("SELECT * FROM warmup_routine_item WHERE day_key = :dayKey ORDER BY order_index ASC")
    suspend fun forDay(dayKey: String): List<WarmupRoutineItem>

    @Query("DELETE FROM warmup_routine_item WHERE day_key = :dayKey")
    suspend fun clearDay(dayKey: String)

    @Query("DELETE FROM warmup_routine_item")
    suspend fun deleteAll()
}
