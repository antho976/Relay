package com.forge.app.ui.gym.train.state

import com.forge.app.program.MuscleGroup

/**
 * Snapshot computed at the moment the workout is finished. Lives only for as long
 * as the summary sheet is visible — not stored. The data that *is* persisted
 * (Session.totalVolumeLb, Session.prCount, individual LoggedExercise.wasPr) is
 * separate and lives in Room.
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
    /** Per-exercise recap rows — name, sets, volume, PR flag (logged, non-skipped only). */
    val highlights: List<ExerciseHighlight>,
    /** Working sets per muscle group THIS session — tints the recap's body map. */
    val setsByMuscle: Map<MuscleGroup, Int> = emptyMap(),
    /** Logged sets carrying an RPE value — the coach's per-set effort signal (data-capture nudge). */
    val setsWithRpe: Int = 0,
    /** Logged (non-skipped) exercises carrying a "how hard it felt" rating (data-capture nudge). */
    val exercisesRated: Int = 0,
    /** The coach's one-line read of this session (#19), or null for an empty session. */
    val coachOpinion: String? = null,
    /** Journal text already captured mid-session (top-bar note) — flows straight through on COMPLETE. */
    val initialJournal: String = "",
    /** Lifetime PR count after this session — drives the PR-milestone notification. */
    val lifetimePrCount: Int = 0
)

data class ExerciseHighlight(
    val exerciseName: String,
    val setsLogged: Int,
    val volumeLb: Double,
    val isPr: Boolean
)
