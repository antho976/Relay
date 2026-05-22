package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * Post-session mood rating (the prototype shows the prompt ~25 % of the time
 * a session is finished with at least one set logged).
 *
 * `sessionId` is nullable: if a session is later deleted, the mood entry survives
 * with its `dayKey` and `recordedAt` intact (`ON DELETE SET NULL`). The data is
 * worth keeping even if the parent workout disappears.
 */
@Entity(
    tableName = "mood_entry",
    foreignKeys = [
        ForeignKey(
            entity = Session::class,
            parentColumns = ["id"],
            childColumns = ["session_id"],
            onDelete = ForeignKey.SET_NULL
        )
    ],
    indices = [Index("session_id")]
)
data class MoodEntry(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "session_id") val sessionId: Long? = null,
    @ColumnInfo(name = "day_key") val dayKey: String,
    @ColumnInfo(name = "mood") val mood: String,
    @ColumnInfo(name = "recorded_at") val recordedAt: Long
)
