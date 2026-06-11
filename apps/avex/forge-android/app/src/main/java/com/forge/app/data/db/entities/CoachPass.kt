package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * One Weekly Coach Pass record (auto-coach Phase 1). Keyed by ISO week id ("2026-W24") so
 * the pass is idempotent by construction — both triggers (first open of a new week, and
 * later end-of-week) no-op when the row exists (hardening decision 10). EVERY pass writes a
 * row, including holds: a missing record means the coach is broken, never "quietly fine"
 * (hardening decisions 4 + 13).
 *
 * Lives in Room (not DataStore) so backup #138 carries program + coach state as one
 * coherent artifact.
 */
@Entity(tableName = "coach_pass")
data class CoachPass(
    @PrimaryKey @ColumnInfo(name = "week_id") val weekId: String,
    @ColumnInfo(name = "ran_at") val ranAt: Long,
    /** "shadow" | "hold" | "error" — Phase 3 adds "proposed"/"applied". ERROR ≠ HOLD, everywhere. */
    @ColumnInfo(name = "status") val status: String,
    /** The explained hold (or the error message) — null for a shadow pass with decisions. */
    @ColumnInfo(name = "hold_reason") val holdReason: String?
)

/**
 * One adjustment a Weekly Coach Pass surfaced. Phase 1 wrote status "shadow" (display-only);
 * Phase 3 writes "proposed" and the user moves it through the lifecycle:
 * proposed → applied (one-tap, via existing user write paths) | skipped; applied → reverted.
 * Applied rows carry [undoData] (the before-state — hardening decision 6's per-change delta)
 * and an [outcome] the watcher fills in after its window (hardening decision 5).
 */
@Entity(tableName = "coach_decision", indices = [Index("week_id")])
data class CoachDecision(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "week_id") val weekId: String,
    /** "deload" | "swap" | "rep_shift" | "weight_nudge" | "volume_up" | "volume_down" | "revert". */
    @ColumnInfo(name = "type") val type: String,
    /** Exercise library id, or "week" for week-level calls. */
    @ColumnInfo(name = "target_key") val targetKey: String,
    @ColumnInfo(name = "target_name") val targetName: String,
    @ColumnInfo(name = "summary") val summary: String,
    @ColumnInfo(name = "reason") val reason: String,
    /** "shadow" (Phase 1) | "proposed" | "applied" | "skipped" | "reverted". */
    @ColumnInfo(name = "status") val status: String,
    /** Day the target slot lives on — apply paths for rep/sets overrides need it. */
    @ColumnInfo(name = "day_key") val dayKey: String = "",
    /** Type-specific apply argument: swap = replacement lib id; volume = new set count; rep_shift = new range; revert = original decision id. */
    @ColumnInfo(name = "payload") val payload: String? = null,
    @ColumnInfo(name = "applied_at") val appliedAt: Long? = null,
    /** Watcher verdict: "pending" until the window closes → "ok" | "failed". */
    @ColumnInfo(name = "outcome") val outcome: String = "pending",
    /** Before-state for single-change undo; null = not undoable (deload regenerates). */
    @ColumnInfo(name = "undo_data") val undoData: String? = null
)
