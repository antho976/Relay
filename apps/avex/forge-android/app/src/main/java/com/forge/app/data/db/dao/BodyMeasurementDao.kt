package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.BodyMeasurementEntry
import kotlinx.coroutines.flow.Flow

@Dao
interface BodyMeasurementDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(entry: BodyMeasurementEntry): Long

    /** Every reading across all types, newest first — the screen groups these by type in memory. */
    @Query("SELECT * FROM body_measurement ORDER BY date_key DESC, recorded_at DESC")
    fun observeAll(): Flow<List<BodyMeasurementEntry>>

    /** The most recent reading for one type (used for the profile-hub summary). */
    @Query("SELECT * FROM body_measurement WHERE type = :type ORDER BY date_key DESC, recorded_at DESC LIMIT 1")
    suspend fun latest(type: String): BodyMeasurementEntry?

    @Query("DELETE FROM body_measurement WHERE id = :id")
    suspend fun delete(id: Long)

    @Query("DELETE FROM body_measurement")
    suspend fun deleteAll()
}
