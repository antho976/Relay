package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.RestDayEntry
import kotlinx.coroutines.flow.Flow

@Dao
interface RestDayDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(entry: RestDayEntry)

    @Query("DELETE FROM rest_day_entry WHERE date_key = :dateKey")
    suspend fun delete(dateKey: String)

    @Query("SELECT * FROM rest_day_entry WHERE date_key = :dateKey")
    suspend fun get(dateKey: String): RestDayEntry?

    @Query("SELECT * FROM rest_day_entry ORDER BY date_key DESC")
    fun observeAll(): Flow<List<RestDayEntry>>
}
