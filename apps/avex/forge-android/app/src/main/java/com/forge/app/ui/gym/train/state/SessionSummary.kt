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
    /**
     * The exercises nothing says the effort of, each with the set that should carry it: the finish
     * summary asks about them in one place, so a rating forgotten mid-session is still one tap away.
     */
    val unrated: List<UnratedLift> = emptyList(),
    /** The coach's one-line read of this session (#19), or null for an empty session. */
    val coachOpinion: String? = null,
    /** Journal text already captured mid-session (top-bar note) — flows straight through on COMPLETE. */
    val initialJournal: String = "",
    /** Lifetime PR count after this session — drives the PR-milestone notification. */
    val lifetimePrCount: Int = 0
)

/** An exercise the session logged without any effort signal, and the set ([setId]) to rate for it. */
data class UnratedLift(val exerciseName: String, val setId: Long)

data class ExerciseHighlight(
    val exerciseName: String,
    val setsLogged: Int,
    val volumeLb: Double,
    val isPr: Boolean
)
