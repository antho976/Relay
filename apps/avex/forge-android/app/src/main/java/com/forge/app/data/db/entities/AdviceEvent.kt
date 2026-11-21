package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * Feedback log for adaptation-engine recommendations: what was surfaced and what the
 * user did with it. Keyed by the recommendation's stable id
 * ([com.forge.app.domain.adapt.Recommendation.id], e.g. "progression.swap.ua3") so later
 * passes can apply cooldowns ("don't re-suggest a swap dismissed twice") and judge which
 * signals earn their confidence.
 *
 * Table ships with the engine's Phase 2 migration; writes land with the Overview coach
 * feed (Phase 4) — the in-session chip is deliberately not logged per render.
 */
@Entity(
    tableName = "advice_event",
    indices = [Index("advice_id")]
)
data class AdviceEvent(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "advice_id") val adviceId: String,
    /** "shown" | "applied" | "dismissed" */
    @ColumnInfo(name = "action") val action: String,
    @ColumnInfo(name = "logged_at") val loggedAt: Long
)
