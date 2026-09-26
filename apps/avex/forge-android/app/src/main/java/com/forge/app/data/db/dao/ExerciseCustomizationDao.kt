package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import com.forge.app.data.db.entities.ExerciseCustomization

@Dao
interface ExerciseCustomizationDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(customization: ExerciseCustomization)

    @Query("DELETE FROM exercise_customization WHERE exercise_id = :exerciseId")
    suspend fun clear(exerciseId: String)

    /**
     * Drop every coach-applied swap — called by a wholesale regenerate (the prefer/avoid bias
     * carries the learning into the new baseline). User swaps survive (auto-coach seam fix: coach
     * swaps were previously immortal and unreconciled, finding 3).
     *
     * The row also holds the user's rest-timer override and pinned note, and a coach swap stamped
     * the whole row coach-owned, so deleting by source wiped them at the next regenerate (audit
     * 2026-09-26, 08). A coach row carrying either keeps them: its swap is blanked and it goes back
     * to the user. A blank swap never locks a slot, so that hands the coach nothing it didn't have.
     */
    @Transaction
    suspend fun clearCoachSwaps() {
        releaseCoachSwapsKeepingUserFields()
        deleteCoachSwapRows()
    }

    @Query(
        "UPDATE exercise_customization SET swapped_name = '', swapped_unit = '', " +
            "swapped_exercise_id = NULL, source = 'user' WHERE source = 'coach' AND " +
            "(rest_timer_override_seconds IS NOT NULL OR TRIM(pinned_note) != '')"
    )
    suspend fun releaseCoachSwapsKeepingUserFields()

    @Query("DELETE FROM exercise_customization WHERE source = 'coach'")
    suspend fun deleteCoachSwapRows()

    @Query("SELECT * FROM exercise_customization WHERE exercise_id = :exerciseId")
    suspend fun get(exerciseId: String): ExerciseCustomization?

    /** All rows — the coach-lock scan (auto-coach Phase 3, hardening decision 9). */
    @Query("SELECT * FROM exercise_customization")
    suspend fun all(): List<ExerciseCustomization>
    // (Removed setRestTimerOverride: a plain UPDATE that no-ops when no row exists yet, and was
    //  unused — CustomizationRepository.setRestTimerOverride does a correct get-then-upsert.)
}
