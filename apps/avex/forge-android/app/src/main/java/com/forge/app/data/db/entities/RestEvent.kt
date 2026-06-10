package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * One realized rest interval (adaptation engine System 2). Opened when a set is logged
 * (the rest timer starts), closed when the *next* set is logged — so [realizedSeconds]
 * is what the user actually rested, vs the [plannedSeconds] the timer prescribed.
 *
 * Captured rather than derived from `completed_at` deltas because timestamps can't see
 * *intent*: a timer skipped at 1:10 remaining is the strongest "prescribed rest is too
 * long" signal there is. [endedBy]: "next_set" (within the timer), "expired" (timer ran
 * out before the next set), "skipped" (user explicitly skipped the timer).
 *
 * An interval whose session is abandoned/finished without another set is never written.
 */
@Entity(
    tableName = "rest_event",
    foreignKeys = [
        ForeignKey(
            entity = Session::class,
            parentColumns = ["id"],
            childColumns = ["session_id"],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index("session_id"), Index("exercise_id")]
)
data class RestEvent(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "session_id") val sessionId: Long,
    @ColumnInfo(name = "exercise_id") val exerciseId: String,
    /** Index of the set that *started* this rest (the set just logged). */
    @ColumnInfo(name = "set_index") val setIndex: Int,
    @ColumnInfo(name = "planned_seconds") val plannedSeconds: Int,
    @ColumnInfo(name = "realized_seconds") val realizedSeconds: Int,
    /** "next_set" | "expired" | "skipped" */
    @ColumnInfo(name = "ended_by") val endedBy: String,
    /** Net seconds added via the +30s control while this rest was running. */
    @ColumnInfo(name = "seconds_added") val secondsAdded: Int,
    @ColumnInfo(name = "logged_at") val loggedAt: Long
)
