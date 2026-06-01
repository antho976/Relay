package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import com.forge.app.data.db.entities.ProgramDay
import com.forge.app.data.db.entities.ProgramSlot
import kotlinx.coroutines.flow.Flow

/** Reads/writes the active program (program-unlock plan, Phase 0). */
@Dao
interface ProgramDao {

    @Query("SELECT * FROM program_day ORDER BY position ASC")
    fun observeDays(): Flow<List<ProgramDay>>

    @Query("SELECT * FROM program_slot ORDER BY day_id ASC, position ASC")
    fun observeSlots(): Flow<List<ProgramSlot>>

    @Query("SELECT * FROM program_day ORDER BY position ASC")
    suspend fun days(): List<ProgramDay>

    @Query("SELECT * FROM program_slot WHERE day_id = :dayId ORDER BY position ASC")
    suspend fun slotsForDay(dayId: String): List<ProgramSlot>

    @Query("SELECT COUNT(*) FROM program_day")
    suspend fun dayCount(): Int

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertDays(days: List<ProgramDay>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertSlots(slots: List<ProgramSlot>)

    @Query("DELETE FROM program_day")
    suspend fun clearDays()

    @Query("DELETE FROM program_slot")
    suspend fun clearSlots()

    /** Replace the entire active program atomically (generate / rotate / custom edit). */
    @Transaction
    suspend fun replaceProgram(days: List<ProgramDay>, slots: List<ProgramSlot>) {
        clearSlots()
        clearDays()
        insertDays(days)
        insertSlots(slots)
    }
}
