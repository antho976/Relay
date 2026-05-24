package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.forge.app.data.db.entities.ExtendedGoal
import kotlinx.coroutines.flow.Flow

@Dao
interface ExtendedGoalDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(goal: ExtendedGoal): Long

    @Update
    suspend fun update(goal: ExtendedGoal)

    @Delete
    suspend fun delete(goal: ExtendedGoal)

    @Query("SELECT * FROM extended_goal ORDER BY created_at DESC")
    fun observeAll(): Flow<List<ExtendedGoal>>

    @Query("SELECT * FROM extended_goal WHERE exercise_id = :exerciseId")
    fun observeForExercise(exerciseId: String): Flow<List<ExtendedGoal>>

    @Query("UPDATE extended_goal SET completed_at = :ts WHERE id = :id")
    suspend fun markComplete(id: Long, ts: Long)

    @Query("DELETE FROM extended_goal")
    suspend fun deleteAll()
}
