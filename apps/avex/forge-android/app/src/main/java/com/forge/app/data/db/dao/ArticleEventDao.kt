package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.ArticleEvent
import kotlinx.coroutines.flow.Flow

/**
 * The Library's append-only read ledger. Nothing here is ever updated — read state is recomputed
 * from the events by `ArticleRegistry`, the same rule `LessonEventDao` follows.
 */
@Dao
interface ArticleEventDao {

    @Insert(onConflict = OnConflictStrategy.ABORT)
    suspend fun insert(event: ArticleEvent): Long

    @Query("SELECT * FROM article_event ORDER BY at_ms ASC")
    suspend fun all(): List<ArticleEvent>

    @Query("SELECT * FROM article_event ORDER BY at_ms ASC")
    fun observeAll(): Flow<List<ArticleEvent>>

    /** True when this article already has an event of this kind — the idempotence guard. */
    @Query("SELECT COUNT(*) > 0 FROM article_event WHERE article_id = :articleId AND kind = :kind")
    suspend fun has(articleId: String, kind: String): Boolean

    @Query("DELETE FROM article_event")
    suspend fun deleteAll()
}
