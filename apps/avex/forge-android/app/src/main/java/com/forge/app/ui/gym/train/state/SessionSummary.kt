package com.forge.app.ui.gym.train.state

import com.forge.app.program.TrophyIcon

/**
 * Snapshot computed at the moment the workout is finished. Lives only for as long
 * as the summary sheet is visible — not stored. The data that *is* persisted
 * (Session.totalVolumeLb, Session.prCount, individual LoggedExercise.wasPr,
 * UnlockedTrophy rows) is separate and lives in Room.
 */
data class SessionSummary(
    val displayName: String,
    val dayWord: String,
    val durationMinutes: Int,
    val totalVolumeLb: Double,
    val prCount: Int,
    val setCount: Int,
    val exercisesLogged: Int,
    val exercisesSkipped: Int,
    val highlights: List<ExerciseHighlight>,
    val unlockedTrophies: List<UnlockedTrophyHighlight> = emptyList(),
    /** Volume delta vs last session on the same day (positive = improvement). Null if no prior session (#52). */
    val vsLastVolumeDelta: Double? = null,
    /** Set count delta vs last session on the same day. Null if no prior session (#52). */
    val vsLastSetsDelta: Int? = null,
    /** True if this session's volume is the all-time best for this day (#53). */
    val isBestSession: Boolean = false,
    /** Sets logged per minute — 0.0 if duration unknown (#83). */
    val setsPerMin: Double = 0.0,
    /** Volume (lb) per minute — 0.0 if duration unknown (#83). */
    val volumePerMin: Double = 0.0,
    /** Volume ÷ duration — the workout density score (#127). Null if duration is 0. */
    val densityScore: Double? = null,
    /** Average rest between consecutive sets within each exercise, in seconds. Null if < 2 sets (#82). */
    val avgRestSeconds: Int? = null,
    /** Planned sets logged / total planned sets × 100. Null if no planned exercises (#133). */
    val honestyPct: Int? = null
)

data class ExerciseHighlight(
    val exerciseName: String,
    val setsLogged: Int,
    val volumeLb: Double,
    val isPr: Boolean
)

/** Just enough to render a row in the summary sheet — the full Trophy lives in the program package. */
data class UnlockedTrophyHighlight(
    val id: String,
    val name: String,
    val description: String,
    val icon: TrophyIcon
)
