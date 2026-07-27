package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

/**
 * The morning check-in (Coach v3 B1): four taps that tell the coach how you actually are, plus the
 * optional extras that only make sense at the same moment.
 *
 * Keyed by ISO calendar date (`yyyy-MM-dd`), one row per day — deliberately NOT a program-day key
 * (`Session.dayKey` is a program-day id like "push", a known foot-gun). Room upserts by that unique
 * index, so answering twice in one morning corrects the day rather than stacking rows.
 *
 * A SKIPPED day still writes a row ([skipped] = true). That's what makes adaptive prompting
 * possible: a user who dismisses it every day should stop being asked, and silence with no record
 * is indistinguishable from never having opened the app.
 *
 * Every field is nullable because every question is optional — four taps is the ceiling, not the
 * requirement, and a partial check-in is worth more than an abandoned one.
 */
@Entity(tableName = "checkin_entry", indices = [Index(value = ["date_key"], unique = true)])
data class CheckinEntry(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** ISO calendar date, "yyyy-MM-dd". */
    @ColumnInfo(name = "date_key") val dateKey: String,
    /** 1 (slept badly) … 5 (slept great); null = not answered. */
    @ColumnInfo(name = "sleep_quality") val sleepQuality: Int? = null,
    /** 1 (no soreness) … 5 (very sore); null = not answered. */
    @ColumnInfo(name = "soreness") val soreness: Int? = null,
    /** 1 (calm) … 5 (very stressed); null = not answered. */
    @ColumnInfo(name = "stress") val stress: Int? = null,
    /** 1 (no drive) … 5 (fired up); null = not answered. */
    @ColumnInfo(name = "motivation") val motivation: Int? = null,
    /** The sick flag — the single source of truth for illness (plan M6). */
    @ColumnInfo(name = "sick") val sick: Boolean = false,
    /**
     * Which muscles are sore, as comma-joined [com.forge.app.program.MuscleGroup] codes. Empty
     * means "sore but unspecified" — per-muscle gates need a per-muscle source, and one generic
     * soreness tap can't provide it.
     */
    @ColumnInfo(name = "sore_muscles") val soreMuscles: String = "",
    /** The user dismissed the sheet. Feeds adaptive prompting; never feeds readiness. */
    @ColumnInfo(name = "skipped") val skipped: Boolean = false,
    @ColumnInfo(name = "recorded_at") val recordedAt: Long
) {
    /** True when at least one question was actually answered. */
    val hasAnswers: Boolean
        get() = !skipped && (sleepQuality != null || soreness != null || stress != null ||
            motivation != null || sick || soreMuscles.isNotEmpty())
}
