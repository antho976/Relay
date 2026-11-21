package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * A timestamped break during an active session (#139).
 * Types: "water" | "rest" | "snack" | "other"
 */
@Entity(
    tableName = "session_break",
    foreignKeys = [ForeignKey(entity = Session::class, parentColumns = ["id"], childColumns = ["session_id"], onDelete = ForeignKey.CASCADE)],
    indices = [Index("session_id")]
)
data class SessionBreak(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "session_id") val sessionId: Long,
    @ColumnInfo(name = "type") val type: String,
    @ColumnInfo(name = "logged_at") val loggedAt: Long
)
