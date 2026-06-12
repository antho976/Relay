package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * One active sitting of a workout. A session can be trained across several sittings — you do
 * some, "resume later", come back and do more — and each sitting is one segment here. The
 * session's real active time is the SUM of segment durations (not wall-clock start→finish,
 * which would count the hours/days you were away). Exports list segments individually so the
 * 13-min + 40-min breakdown is preserved.
 *
 * [endedAt] is null while a sitting is in progress. Cascades on session delete (a discarded
 * session leaves no orphan segments).
 */
@Entity(
    tableName = "session_segment",
    foreignKeys = [ForeignKey(
        entity = Session::class,
        parentColumns = ["id"],
        childColumns = ["session_id"],
        onDelete = ForeignKey.CASCADE
    )],
    indices = [Index("session_id")]
)
data class SessionSegment(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "session_id") val sessionId: Long,
    @ColumnInfo(name = "started_at") val startedAt: Long,
    @ColumnInfo(name = "ended_at") val endedAt: Long? = null
)
