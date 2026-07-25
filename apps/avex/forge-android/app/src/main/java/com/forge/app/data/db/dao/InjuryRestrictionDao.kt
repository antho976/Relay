package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.InjuryRestriction
import kotlinx.coroutines.flow.Flow

/** Injury restrictions (Coach v3 B1). Cleared rows are kept — they explain that month's numbers. */
@Dao
interface InjuryRestrictionDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(restriction: InjuryRestriction): Long

    @Query("SELECT * FROM injury_restriction WHERE cleared_at IS NULL ORDER BY started_at DESC")
    suspend fun active(): List<InjuryRestriction>

    @Query("SELECT * FROM injury_restriction WHERE cleared_at IS NULL ORDER BY started_at DESC")
    fun observeActive(): Flow<List<InjuryRestriction>>

    @Query("SELECT * FROM injury_restriction ORDER BY started_at DESC")
    suspend fun all(): List<InjuryRestriction>

    @Query("UPDATE injury_restriction SET cleared_at = :atMs WHERE id = :id")
    suspend fun clear(id: Long, atMs: Long)

    @Query("DELETE FROM injury_restriction WHERE id = :id")
    suspend fun delete(id: Long)
}
