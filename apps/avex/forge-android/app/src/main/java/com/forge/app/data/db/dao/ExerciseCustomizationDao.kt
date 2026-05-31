package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.ExerciseCustomization

@Dao
interface ExerciseCustomizationDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(customization: ExerciseCustomization)

    @Query("DELETE FROM exercise_customization WHERE exercise_id = :exerciseId")
    suspend fun clear(exerciseId: String)

    @Query("SELECT * FROM exercise_customization WHERE exercise_id = :exerciseId")
    suspend fun get(exerciseId: String): ExerciseCustomization?

    /** Upsert only the rest timer override, preserving existing swap name/unit (#59). */
    @Query("UPDATE exercise_customization SET rest_timer_override_seconds = :seconds WHERE exercise_id = :exerciseId")
    suspend fun setRestTimerOverride(exerciseId: String, seconds: Int?)
}
