package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.CoachProject
import kotlinx.coroutines.flow.Flow

/** Proactive projects (Coach v3 D). At most one active at a time; the rest are the record. */
@Dao
interface CoachProjectDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(project: CoachProject): Long

    @Query("SELECT * FROM coach_project WHERE completed_at IS NULL AND abandoned_at IS NULL ORDER BY started_at DESC LIMIT 1")
    suspend fun active(): CoachProject?

    @Query("SELECT * FROM coach_project WHERE completed_at IS NULL AND abandoned_at IS NULL ORDER BY started_at DESC LIMIT 1")
    fun observeActive(): Flow<CoachProject?>

    @Query("SELECT * FROM coach_project ORDER BY started_at DESC")
    suspend fun all(): List<CoachProject>

    @Query("UPDATE coach_project SET completed_at = :atMs WHERE id = :id")
    suspend fun markCompleted(id: Long, atMs: Long)

    @Query("UPDATE coach_project SET abandoned_at = :atMs WHERE id = :id")
    suspend fun markAbandoned(id: Long, atMs: Long)
}
