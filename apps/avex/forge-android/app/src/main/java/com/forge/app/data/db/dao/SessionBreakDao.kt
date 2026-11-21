package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import com.forge.app.data.db.entities.SessionBreak

/**
 * Write-only on purpose, for now.
 *
 * `WorkoutRepository.logBreak` stamps a row per break taken during a session and nothing reads
 * `session_break` back yet — no query, no delete. That is a deliberate hold, not an oversight: the
 * rows are the raw material for the "where did the time go" reading of a session, they are bounded
 * by session count (the FK cascades them away with their session), and dropping the write to retire
 * the table would throw away history that can't be reconstructed later. Kept, documented, and read
 * when the surface that wants it ships.
 */
@Dao
interface SessionBreakDao {
    @Insert
    suspend fun insert(brk: SessionBreak): Long
}
