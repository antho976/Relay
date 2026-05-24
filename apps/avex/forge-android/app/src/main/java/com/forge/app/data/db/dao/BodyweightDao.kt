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

    @Query("SELECT * FROM bodyweight_entry ORDER BY date_key DESC LIMIT 1")
    suspend fun latest(): BodyweightEntry?

    @Query("DELETE FROM bodyweight_entry WHERE id = :id")
    suspend fun delete(id: Long)

    @Query("DELETE FROM bodyweight_entry")
    suspend fun deleteAll()
}
