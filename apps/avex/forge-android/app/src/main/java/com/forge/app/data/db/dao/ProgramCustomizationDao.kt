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

    @Query("SELECT * FROM program_customization")
    suspend fun all(): List<ProgramCustomization>

    /** Active, user-created (custom_…) rows only — feeds the likeable custom-exercise list. Scoped so
     *  the scan skips swap/override rows; downstream still needs distinctUntilChanged because Room
     *  invalidates this Flow at table granularity (every program_customization write re-emits). */
    @Query("SELECT * FROM program_customization WHERE removed = 0 AND exercise_id LIKE 'custom\\_%' ESCAPE '\\'")
    fun observeCustom(): Flow<List<ProgramCustomization>>

    @Query("DELETE FROM program_customization WHERE day_key = :dayKey AND exercise_id = :exerciseId")
    suspend fun delete(dayKey: String, exerciseId: String)

    @Query("DELETE FROM program_customization WHERE day_key = :dayKey")
    suspend fun clearDay(dayKey: String)

    @Query("DELETE FROM program_customization")
    suspend fun deleteAll()
}
