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

    /**
     * The FIRST weigh-in recorded at or after [sinceMs] — the start of a journey that began then
     * (M-33), as opposed to [latest], which is where the user is now.
     *
     * A bodyweight goal created before any weigh-in has no baseline, and adopting it from [latest]
     * meant the start was whatever the scale said the first time the goal happened to be READ. Log
     * 200 and then 190 before opening the page and 190 was stored as where the journey began.
     */
    @Query("SELECT * FROM bodyweight_entry WHERE recorded_at >= :sinceMs ORDER BY recorded_at ASC, date_key ASC LIMIT 1")
    suspend fun earliestSince(sinceMs: Long): BodyweightEntry?

    /**
     * That day's row, or null. Needed because [upsert] is INSERT OR REPLACE: on a `date_key`
     * conflict SQLite DELETES the existing row and inserts a new one, so any column the caller
     * does not set is lost and the row gets a fresh id. Callers that mean to update rather than
     * overwrite read the row first and carry forward what they are not changing.
     */
    @Query("SELECT * FROM bodyweight_entry WHERE date_key = :dateKey LIMIT 1")
    suspend fun byDateKey(dateKey: String): BodyweightEntry?

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

    /** One row by id, read before a delete so its Health Connect mirror (keyed on `date_key`) can go too. */
    @Query("SELECT * FROM bodyweight_entry WHERE id = :id LIMIT 1")
    suspend fun byId(id: Long): BodyweightEntry?

    @Query("DELETE FROM bodyweight_entry WHERE id = :id")
    suspend fun delete(id: Long)

    @Query("DELETE FROM bodyweight_entry")
    suspend fun deleteAll()
}
