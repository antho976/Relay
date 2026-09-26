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

    /** Full history, newest-first — the cardio log list is no longer capped at 20. */
    @Query("SELECT * FROM cardio_entry ORDER BY date DESC")
    fun observeAll(): Flow<List<CardioEntry>>

    @Query("SELECT * FROM cardio_entry WHERE date >= :since ORDER BY date DESC")
    fun observeSince(since: Long): Flow<List<CardioEntry>>

    @Query("SELECT * FROM cardio_entry WHERE date >= :since ORDER BY date DESC")
    suspend fun since(since: Long): List<CardioEntry>

    /** Non-rest entries in [start, end), oldest-first — a single week's slice for the PDF report. */
    @Query("SELECT * FROM cardio_entry WHERE date >= :start AND date < :end AND type != :excludeType ORDER BY date")
    suspend fun between(start: Long, end: Long, excludeType: String = "rest"): List<CardioEntry>

    /** Cardio minutes since [sinceEpochMs], excluding rest-day entries. */
    @Query("SELECT SUM(duration_min) FROM cardio_entry WHERE date >= :sinceEpochMs AND type != :excludeType")
    fun observeMinutesSince(sinceEpochMs: Long, excludeType: String = "rest"): Flow<Int?>

    /** Cardio distance (km) since [sinceEpochMs], excluding rest-day entries. */
    @Query("SELECT SUM(distance_km) FROM cardio_entry WHERE date >= :sinceEpochMs AND type != :excludeType AND distance_km IS NOT NULL")
    fun observeDistanceKmSince(sinceEpochMs: Long, excludeType: String = "rest"): Flow<Double?>

    /** Count of non-rest cardio sessions ever (cardio trophies). */
    @Query("SELECT COUNT(*) FROM cardio_entry WHERE type != :excludeType")
    suspend fun totalSessions(excludeType: String = "rest"): Int

    /** Total cardio distance (km) ever, excluding rest entries (cardio trophies). */
    @Query("SELECT SUM(distance_km) FROM cardio_entry WHERE type != :excludeType AND distance_km IS NOT NULL")
    suspend fun totalDistanceKm(excludeType: String = "rest"): Double?

    /** Total active cardio minutes ever, excluding rest entries (profile all-time totals). */
    @Query("SELECT SUM(duration_min) FROM cardio_entry WHERE type != :excludeType")
    suspend fun totalMinutes(excludeType: String = "rest"): Int?

    /**
     * Duplicate guard for the importer: entries at the same instant with the same type, duration and
     * distance. Distance is part of it because a date-only export puts every entry that day at
     * midnight, and a 30-minute 4 km run then collapsed into a 30-minute 5 km one. Returns ids, not
     * a yes/no, so the caller can let one stored entry stand in for exactly one incoming one.
     */
    @Query(
        """SELECT id FROM cardio_entry WHERE date = :date AND type = :type AND duration_min = :durationMin
           AND (distance_km IS :distanceKm OR ABS(distance_km - :distanceKm) < 0.001) ORDER BY id"""
    )
    suspend fun idsLike(date: Long, type: String, durationMin: Int, distanceKm: Double?): List<Long>

    /** Every entry id, captured by the reset BEFORE [deleteAll] so the Health Connect mirrors keyed
     *  on them can still be addressed once the rows are gone (M-02). */
    @Query("SELECT id FROM cardio_entry")
    suspend fun allIds(): List<Long>

    @Query("DELETE FROM cardio_entry")
    suspend fun deleteAll()
}
