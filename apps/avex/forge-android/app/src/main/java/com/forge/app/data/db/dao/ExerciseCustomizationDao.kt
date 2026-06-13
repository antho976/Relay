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

    /**
     * Drop every coach-applied swap row — called by a wholesale regenerate (the prefer/avoid bias
     * carries the learning into the new baseline). User swaps survive (auto-coach seam fix: coach
     * swaps were previously immortal and unreconciled, finding 3).
     */
    @Query("DELETE FROM exercise_customization WHERE source = 'coach'")
    suspend fun clearCoachSwaps()

    @Query("SELECT * FROM exercise_customization WHERE exercise_id = :exerciseId")
    suspend fun get(exerciseId: String): ExerciseCustomization?

    /** All rows — the coach-lock scan (auto-coach Phase 3, hardening decision 9). */
    @Query("SELECT * FROM exercise_customization")
    suspend fun all(): List<ExerciseCustomization>
    // (Removed setRestTimerOverride: a plain UPDATE that no-ops when no row exists yet, and was
    //  unused — CustomizationRepository.setRestTimerOverride does a correct get-then-upsert.)
}
