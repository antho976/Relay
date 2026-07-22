package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.BodyFatEntry
import kotlinx.coroutines.flow.Flow

@Dao
interface BodyFatDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(entry: BodyFatEntry): Long

    @Query("SELECT * FROM body_fat ORDER BY date_key DESC LIMIT :limit")
    fun observeRecent(limit: Int = 90): Flow<List<BodyFatEntry>>

    @Query("SELECT * FROM body_fat ORDER BY date_key DESC, recorded_at DESC LIMIT 1")
    suspend fun latest(): BodyFatEntry?

    /** Every reading, newest first — for the body-fat CSV export. */
    @Query("SELECT * FROM body_fat ORDER BY date_key DESC, recorded_at DESC")
    suspend fun all(): List<BodyFatEntry>

    @Query("DELETE FROM body_fat WHERE id = :id")
    suspend fun delete(id: Long)

    @Query("DELETE FROM body_fat")
    suspend fun deleteAll()
}
