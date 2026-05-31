package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import com.forge.app.data.db.entities.SessionBreak

@Dao
interface SessionBreakDao {
    @Insert
    suspend fun insert(brk: SessionBreak): Long
}
