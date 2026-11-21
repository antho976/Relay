package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.ExtendedGoal
import kotlinx.coroutines.flow.Flow

@Dao
interface ExtendedGoalDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(goal: ExtendedGoal): Long

    @Query("SELECT * FROM extended_goal ORDER BY created_at DESC")
    fun observeAll(): Flow<List<ExtendedGoal>>

    @Query("SELECT * FROM extended_goal ORDER BY created_at DESC")
    suspend fun getAll(): List<ExtendedGoal>

    @Query("SELECT * FROM extended_goal WHERE id = :id")
    suspend fun getById(id: Long): ExtendedGoal?

    @Query("UPDATE extended_goal SET target_value = :target WHERE id = :id")
    suspend fun updateTarget(id: Long, target: Double)

    /**
     * Adopt [baseline] as a BODYWEIGHT goal's starting weight, but only while it has none — the
     * journey's start is written once and must never move afterwards (M-33). The `WHERE ... IS NULL`
     * is what makes it once: two concurrent reads racing to adopt the first weigh-in cannot each
     * write a different one.
     */
    @Query("UPDATE extended_goal SET stretch_value = :baseline WHERE id = :id AND stretch_value IS NULL")
    suspend fun adoptBaselineIfMissing(id: Long, baseline: Double)

    @Query("DELETE FROM extended_goal WHERE id = :id")
    suspend fun deleteById(id: Long)
}
