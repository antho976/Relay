package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.LeanMassEntry
import kotlinx.coroutines.flow.Flow

@Dao
interface LeanMassDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(entry: LeanMassEntry): Long

    @Query("SELECT * FROM lean_mass ORDER BY date_key DESC LIMIT :limit")
    fun observeRecent(limit: Int = 90): Flow<List<LeanMassEntry>>

    @Query("SELECT * FROM lean_mass ORDER BY date_key DESC, recorded_at DESC LIMIT 1")
    suspend fun latest(): LeanMassEntry?

    @Query("DELETE FROM lean_mass WHERE id = :id")
    suspend fun delete(id: Long)

    @Query("DELETE FROM lean_mass")
    suspend fun deleteAll()
}
