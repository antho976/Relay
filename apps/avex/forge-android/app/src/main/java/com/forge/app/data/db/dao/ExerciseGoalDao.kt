package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.ExerciseGoal
import kotlinx.coroutines.flow.Flow

@Dao
interface ExerciseGoalDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(goal: ExerciseGoal)

    @Query("SELECT * FROM exercise_goal WHERE exercise_id = :exerciseId")
    fun observe(exerciseId: String): Flow<ExerciseGoal?>

    @Query("SELECT * FROM exercise_goal WHERE exercise_id = :exerciseId")
    suspend fun get(exerciseId: String): ExerciseGoal?

    @Query("DELETE FROM exercise_goal WHERE exercise_id = :exerciseId")
    suspend fun delete(exerciseId: String)

    @Query("SELECT * FROM exercise_goal")
    fun observeAll(): Flow<List<ExerciseGoal>>
}
