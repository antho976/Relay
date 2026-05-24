package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.VacationPeriod
import kotlinx.coroutines.flow.Flow

@Dao
interface VacationDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(period: VacationPeriod): Long

    @Delete
    suspend fun delete(period: VacationPeriod)

    @Query("SELECT * FROM vacation_period ORDER BY start_date DESC")
    fun observeAll(): Flow<List<VacationPeriod>>

    @Query("SELECT * FROM vacation_period ORDER BY start_date DESC")
    suspend fun all(): List<VacationPeriod>

    @Query("DELETE FROM vacation_period")
    suspend fun deleteAll()
}
