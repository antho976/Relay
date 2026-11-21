package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index

/**
 * One live heart-rate sample streamed from the watch during a session (W3). Batched over the wear
 * protocol (~5 s), written by WearHrIngest, and consumed post-session: the session-detail HR graph
 * (set markers + per-exercise bands), the between-set HRR stat, and the HC write-back's HR series.
 * CASCADE with its session — deleting a workout takes its HR trace with it.
 */
@Entity(
    tableName = "session_hr_sample",
    primaryKeys = ["session_id", "at_ms"],
    foreignKeys = [
        ForeignKey(
            entity = Session::class,
            parentColumns = ["id"],
            childColumns = ["session_id"],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index("session_id")]
)
data class SessionHrSample(
    @ColumnInfo(name = "session_id") val sessionId: Long,
    @ColumnInfo(name = "at_ms") val atMs: Long,
    @ColumnInfo(name = "bpm") val bpm: Int
)
