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
    val unlockedTrophies: List<UnlockedTrophyHighlight> = emptyList()
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
