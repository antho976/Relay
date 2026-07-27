package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.LessonEvent
import kotlinx.coroutines.flow.Flow

/**
 * The Academy's append-only read ledger (Coach v3 A2). Nothing here is ever updated — unlock and
 * read state are recomputed from the events by `AcademyRegistry`, the same idempotent rule the
 * coach's bias uses.
 */
@Dao
interface LessonEventDao {

    @Insert(onConflict = OnConflictStrategy.ABORT)
    suspend fun insert(event: LessonEvent): Long

    @Query("SELECT * FROM lesson_event ORDER BY at_ms ASC")
    suspend fun all(): List<LessonEvent>

    @Query("SELECT * FROM lesson_event ORDER BY at_ms ASC")
    fun observeAll(): Flow<List<LessonEvent>>

    /** True when this lesson already has an event of this kind — the idempotence guard for unlocks. */
    @Query("SELECT COUNT(*) > 0 FROM lesson_event WHERE lesson_id = :lessonId AND kind = :kind")
    suspend fun has(lessonId: String, kind: String): Boolean

    @Query("DELETE FROM lesson_event")
    suspend fun deleteAll()
}
