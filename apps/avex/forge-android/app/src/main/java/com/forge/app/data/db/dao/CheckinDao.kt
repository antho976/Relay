package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.CheckinEntry
import kotlinx.coroutines.flow.Flow

/** The daily check-in (Coach v3 B1). One row per ISO day; REPLACE upserts by `date_key`. */
@Dao
interface CheckinDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(entry: CheckinEntry): Long

    @Query("SELECT * FROM checkin_entry WHERE date_key = :dateKey LIMIT 1")
    suspend fun forDate(dateKey: String): CheckinEntry?

    @Query("SELECT * FROM checkin_entry WHERE date_key = :dateKey LIMIT 1")
    fun observeForDate(dateKey: String): Flow<CheckinEntry?>

    /** Newest first, windowed — readiness reads days, not history. */
    @Query("SELECT * FROM checkin_entry WHERE recorded_at >= :sinceMs ORDER BY recorded_at DESC")
    suspend fun since(sinceMs: Long): List<CheckinEntry>

    @Query("SELECT * FROM checkin_entry ORDER BY date_key DESC")
    suspend fun all(): List<CheckinEntry>

    @Query("DELETE FROM checkin_entry WHERE id = :id")
    suspend fun delete(id: Long)
}
