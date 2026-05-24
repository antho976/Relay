package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.Query
import androidx.room.Update
import com.forge.app.data.db.entities.CardioEntry
import kotlinx.coroutines.flow.Flow

@Dao
interface CardioDao {

    @Insert
    suspend fun insert(entry: CardioEntry): Long

    @Update
    suspend fun update(entry: CardioEntry)

    @Delete
    suspend fun delete(entry: CardioEntry)

    @Query("SELECT * FROM cardio_entry WHERE id = :id")
    suspend fun get(id: Long): CardioEntry?

    @Query("SELECT * FROM cardio_entry ORDER BY date DESC LIMIT :limit")
    fun observeRecent(limit: Int = 20): Flow<List<CardioEntry>>

    @Query("SELECT * FROM cardio_entry WHERE date >= :since ORDER BY date DESC")
    fun observeSince(since: Long): Flow<List<CardioEntry>>

    @Query("SELECT * FROM cardio_entry WHERE date >= :since ORDER BY date DESC")
    suspend fun since(since: Long): List<CardioEntry>

    /** Cardio minutes since [sinceEpochMs], excluding rest-day entries. */
    @Query("SELECT SUM(duration_min) FROM cardio_entry WHERE date >= :sinceEpochMs AND type != :excludeType")
    fun observeMinutesSince(sinceEpochMs: Long, excludeType: String = "rest"): Flow<Int?>

    /** Cardio distance (km) since [sinceEpochMs], excluding rest-day entries. */
    @Query("SELECT SUM(distance_km) FROM cardio_entry WHERE date >= :sinceEpochMs AND type != :excludeType AND distance_km IS NOT NULL")
    fun observeDistanceKmSince(sinceEpochMs: Long, excludeType: String = "rest"): Flow<Double?>

    @Query("DELETE FROM cardio_entry")
    suspend fun deleteAll()

    /** Sum of all distance_km across all entries — lifetime total (#79). */
    @Query("SELECT SUM(distance_km) FROM cardio_entry WHERE distance_km IS NOT NULL")
    fun observeLifetimeDistanceKm(): Flow<Double?>

    /** All Run entries with both distance and duration > 0, ordered by date, for pace trend (#78). */
    @Query("SELECT * FROM cardio_entry WHERE type = 'run' AND distance_km > 0 AND duration_min > 0 ORDER BY date ASC")
    fun observeRunEntries(): Flow<List<CardioEntry>>
}
