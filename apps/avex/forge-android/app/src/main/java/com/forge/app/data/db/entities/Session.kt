package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * A single workout. `finishedAt` is null while the workout is in progress —
 * there should be at most one such row at any time (enforced by app logic
 * via [com.forge.app.data.db.dao.SessionDao.getActiveSession]).
 *
 * `totalVolumeLb` and `prCount` are denormalised on session finish, so the
 * overview / history screens can list sessions cheaply without re-joining
 * exercises and sets.
 */
@Entity(tableName = "session")
data class Session(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "day_key") val dayKey: String,
    @ColumnInfo(name = "started_at") val startedAt: Long,
    @ColumnInfo(name = "finished_at") val finishedAt: Long? = null,
    @ColumnInfo(name = "total_volume_lb") val totalVolumeLb: Double? = null,
    @ColumnInfo(name = "pr_count") val prCount: Int = 0,
    @ColumnInfo(name = "set_count") val setCount: Int = 0,
    @ColumnInfo(name = "deload_marked_here") val deloadMarkedHere: Boolean = false,
    /** Comma-separated quick tags chosen at session end (#107). Empty string = no tags. */
    @ColumnInfo(name = "tags") val tags: String = "",
    /** Session type marker (#109): "normal" | "deload" | "test" | "technique" | "first_back" */
    @ColumnInfo(name = "session_type") val sessionType: String = "normal",
    /** If true, session is untracked — excluded from streak, trophies, suggestions (#110). */
    @ColumnInfo(name = "is_untracked") val isUntracked: Boolean = false,
    /** Free-text journal for the whole session (#111). */
    @ColumnInfo(name = "journal") val journal: String = "",
    /** Intensity intent (#123): "light" | "normal" | "hard" */
    @ColumnInfo(name = "intensity") val intensity: String = "normal"
)
