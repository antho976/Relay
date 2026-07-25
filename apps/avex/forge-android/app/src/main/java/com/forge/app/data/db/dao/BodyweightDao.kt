package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.BodyweightEntry
import kotlinx.coroutines.flow.Flow

@Dao
interface BodyweightDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(entry: BodyweightEntry): Long

    @Query("SELECT * FROM bodyweight_entry ORDER BY date_key DESC LIMIT :limit")
    fun observeRecent(limit: Int = 90): Flow<List<BodyweightEntry>>

    @Query("SELECT * FROM bodyweight_entry ORDER BY date_key DESC, recorded_at DESC LIMIT 1")
    suspend fun latest(): BodyweightEntry?

    /** Every weigh-in, newest first — for the bodyweight CSV export. */
    @Query("SELECT * FROM bodyweight_entry ORDER BY date_key DESC, recorded_at DESC")
    suspend fun all(): List<BodyweightEntry>

    /**
     * Weigh-ins recorded at/after [sinceMs], newest first — the adaptation snapshot's bodyweight
     * series (A1). Windowed like the mood/cardio reads so the engine never loads a whole history
     * it can't use. Filters on `recorded_at` (an epoch-ms column) rather than the ISO `date_key`,
     * so the comparison is numeric.
     */
    @Query("SELECT * FROM bodyweight_entry WHERE recorded_at >= :sinceMs ORDER BY recorded_at DESC")
    suspend fun since(sinceMs: Long): List<BodyweightEntry>

    @Query("DELETE FROM bodyweight_entry WHERE id = :id")
    suspend fun delete(id: Long)

    @Query("DELETE FROM bodyweight_entry")
    suspend fun deleteAll()
}
