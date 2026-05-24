package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.ProgramCustomization
import kotlinx.coroutines.flow.Flow

@Dao
interface ProgramCustomizationDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(c: ProgramCustomization)

    @Query("SELECT * FROM program_customization WHERE day_key = :dayKey ORDER BY order_override ASC")
    fun observeForDay(dayKey: String): Flow<List<ProgramCustomization>>

    @Query("SELECT * FROM program_customization WHERE day_key = :dayKey ORDER BY order_override ASC")
    suspend fun forDay(dayKey: String): List<ProgramCustomization>

    @Query("DELETE FROM program_customization WHERE day_key = :dayKey AND exercise_id = :exerciseId")
    suspend fun delete(dayKey: String, exerciseId: String)

    @Query("DELETE FROM program_customization WHERE day_key = :dayKey")
    suspend fun clearDay(dayKey: String)

    @Query("DELETE FROM program_customization")
    suspend fun deleteAll()
}
