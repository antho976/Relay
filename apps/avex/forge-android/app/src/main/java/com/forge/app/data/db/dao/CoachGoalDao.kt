package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.forge.app.data.db.entities.CoachGoal
import kotlinx.coroutines.flow.Flow

/** The Goal Portfolio's rows (Coach v3 A2). Active goals sort by priority, then by age. */
@Dao
interface CoachGoalDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(goal: CoachGoal): Long

    @Update
    suspend fun update(goal: CoachGoal)

    @Query("SELECT * FROM coach_goal ORDER BY priority ASC, created_at ASC")
    suspend fun all(): List<CoachGoal>

    @Query(
        "SELECT * FROM coach_goal WHERE completed_at IS NULL AND archived_at IS NULL " +
            "ORDER BY priority ASC, created_at ASC"
    )
    suspend fun active(): List<CoachGoal>

    @Query(
        "SELECT * FROM coach_goal WHERE completed_at IS NULL AND archived_at IS NULL " +
            "ORDER BY priority ASC, created_at ASC"
    )
    fun observeActive(): Flow<List<CoachGoal>>

    @Query("SELECT * FROM coach_goal WHERE id = :id")
    suspend fun byId(id: Long): CoachGoal?

    @Query("UPDATE coach_goal SET completed_at = :atMs WHERE id = :id")
    suspend fun markCompleted(id: Long, atMs: Long)

    @Query("UPDATE coach_goal SET archived_at = :atMs WHERE id = :id")
    suspend fun markArchived(id: Long, atMs: Long)

    @Query("UPDATE coach_goal SET priority = :priority WHERE id = :id")
    suspend fun setPriority(id: Long, priority: Int)

    @Query("DELETE FROM coach_goal WHERE id = :id")
    suspend fun delete(id: Long)
}
