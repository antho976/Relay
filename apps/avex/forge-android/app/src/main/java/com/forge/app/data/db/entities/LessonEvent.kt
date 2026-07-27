package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * One Academy moment (Coach v3 A2): a lesson was unlocked, opened, or finished.
 *
 * Lessons themselves are static in-app content (`domain/academy/`), not rows — only the user's
 * relationship to them is stored, and it's stored as an append-only LEDGER for the same reason the
 * coach's decisions are: unlock state is *recomputed* from events (`AcademyRegistry.stateFrom`),
 * never mutated in place, so it can't drift or corrupt.
 *
 * The ledger exists because half the curriculum's triggers are app-usage moments (first rest-timer
 * use, first readiness tap) that write no coach row — the coach ledger alone could not answer
 * "has this unlocked?".
 */
@Entity(tableName = "lesson_event", indices = [Index("lesson_id")])
data class LessonEvent(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** Stable content id, e.g. "coach.strength_on_a_cut". */
    @ColumnInfo(name = "lesson_id") val lessonId: String,
    /** [com.forge.app.domain.academy.LessonEventKind.code] — "unlocked" | "opened" | "completed". */
    @ColumnInfo(name = "kind") val kind: String,
    @ColumnInfo(name = "at_ms") val atMs: Long
)
