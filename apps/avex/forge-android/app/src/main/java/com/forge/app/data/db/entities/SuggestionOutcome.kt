package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * Outcome of one weight suggestion (auto-coach Phase 2 — calibration signal): what the chip
 * suggested vs what the user actually lifted on their FIRST set of that exercise, and how
 * the set went. One row per exercise per session, written only when a suggestion was showing.
 *
 * The calibrator ([com.forge.app.domain.coach.SuggestionCalibrator]) reads these to learn
 * per-exercise step behavior — consistently overriding upward and succeeding earns bigger
 * steps; taking suggestions that then fail earns a consolidation hold. Raw facts only;
 * judgments live in the pure calibrator.
 */
@Entity(
    tableName = "suggestion_outcome",
    indices = [Index("exercise_id")]
)
data class SuggestionOutcome(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "exercise_id") val exerciseId: String,
    /** [com.forge.app.program.ExerciseUnit.code] — "db" | "plates". */
    @ColumnInfo(name = "unit") val unit: String,
    @ColumnInfo(name = "suggested_lb") val suggestedLb: Double,
    @ColumnInfo(name = "taken_lb") val takenLb: Double,
    /** Reps achieved on that first set. */
    @ColumnInfo(name = "reps") val reps: Int,
    /** The slot's rep prescription at the time ("8-12") — success is judged against its floor. */
    @ColumnInfo(name = "range_text") val rangeText: String,
    @ColumnInfo(name = "logged_at") val loggedAt: Long
)
