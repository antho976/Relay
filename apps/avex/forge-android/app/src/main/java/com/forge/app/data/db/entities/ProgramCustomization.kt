package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * Persisted program customizations — overlays applied on top of the active program (#90, #91, #92).
 * One row per exercise per day, keyed by (day_key, exercise_id). The exercise can be:
 *   - from the generated/static program (exerciseId is the ExerciseLibrary id, e.g. "db-bench-press")
 *   - added by the user (exerciseId is "custom_<uuid>", customName is non-null)
 *
 * These overlays are NOT tied to a program generation: a wholesale regenerate/reroll clears the
 * static overrides (see ProgramRepository.reconcileCustomizations) so a stale override can't re-bind
 * to a freshly generated exercise; user-added "custom_" rows survive while their day still exists.
 *
 * If [removed] is true, the exercise is hidden from the day.
 * [repRangeOverride] replaces the plan's reps string (e.g. "6-8" instead of "8-10").
 * [setsOverride] overrides the planned set count (0 = use plan default).
 * [orderOverride] drives the display order within the day (lower = earlier).
 * [source] tags who wrote the row ([OverlaySource]) — the coach-lock scan and per-change undo
 *   need to tell a user's own edit from a coach-applied overlay (auto-coach seam audit findings 5/6).
 */
@Entity(tableName = "program_customization", primaryKeys = ["day_key", "exercise_id"])
data class ProgramCustomization(
    @ColumnInfo(name = "day_key") val dayKey: String,
    @ColumnInfo(name = "exercise_id") val exerciseId: String,
    @ColumnInfo(name = "custom_name") val customName: String? = null,
    @ColumnInfo(name = "custom_muscle") val customMuscle: String? = null,
    @ColumnInfo(name = "rep_range_override") val repRangeOverride: String? = null,
    @ColumnInfo(name = "sets_override") val setsOverride: Int = 0,
    @ColumnInfo(name = "order_override") val orderOverride: Int = 999,
    @ColumnInfo(name = "removed") val removed: Boolean = false,
    @ColumnInfo(name = "source") val source: String = OverlaySource.USER
)
