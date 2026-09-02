package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.forge.app.data.db.entities.TrainingBlock
import kotlinx.coroutines.flow.Flow

/** The training block (Coach v3 C). At most one active block at a time. */
@Dao
interface TrainingBlockDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(block: TrainingBlock): Long

    @Update
    suspend fun update(block: TrainingBlock)

    @Query("SELECT * FROM training_block WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1")
    suspend fun active(): TrainingBlock?

    @Query("SELECT * FROM training_block WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1")
    fun observeActive(): Flow<TrainingBlock?>

    /**
     * Every row still open, in the same order [active] reads them. The invariant says there is at
     * most one; the repository's repair path reads them all so it can end the extras (M-13).
     */
    @Query("SELECT * FROM training_block WHERE ended_at IS NULL ORDER BY started_at DESC")
    suspend fun allActive(): List<TrainingBlock>

    /** End every open row except [keepId], reporting how many were closed. */
    @Query("UPDATE training_block SET ended_at = :endedAt WHERE ended_at IS NULL AND id != :keepId")
    suspend fun endAllExcept(keepId: Long, endedAt: Long): Int

    /** Finished blocks, newest first — the record of how the training year was shaped. */
    @Query("SELECT * FROM training_block ORDER BY started_at DESC")
    suspend fun all(): List<TrainingBlock>
}
