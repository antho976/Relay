package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import com.forge.app.data.db.entities.WearCommand

@Dao
interface WearCommandDao {
    @Query("SELECT * FROM wear_command WHERE command_id = :id")
    suspend fun get(id: String): WearCommand?
    @Insert suspend fun insert(command: WearCommand)
    /** Retention: a command id is only replayed by a watch retry, which happens within minutes. */
    @Query("DELETE FROM wear_command WHERE completed_at < :cutoff")
    suspend fun deleteOlderThan(cutoff: Long): Int
}
