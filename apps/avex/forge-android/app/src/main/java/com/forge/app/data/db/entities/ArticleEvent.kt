package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * One Library moment: an article was opened, or read to the end.
 *
 * The same shape and the same reasoning as [LessonEvent]. Articles are static in-app content
 * (`domain/academy/`), never rows; only the reader's relationship to them is stored, and it is
 * stored as an append-only LEDGER so state is recomputed by `ArticleRegistry.stateFrom` rather than
 * mutated in place.
 *
 * There is no `unlocked` kind, and there never will be. Every article is readable from install, so
 * the only events that exist are the two the reader causes.
 */
@Entity(tableName = "article_event", indices = [Index("article_id")])
data class ArticleEvent(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** Stable content id, always `library.<slug>`. */
    @ColumnInfo(name = "article_id") val articleId: String,
    /** [com.forge.app.domain.academy.ArticleEventKind.code] — "opened" | "finished". */
    @ColumnInfo(name = "kind") val kind: String,
    @ColumnInfo(name = "at_ms") val atMs: Long
)
