package com.forge.app.ui.gym.train.state

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.timer.RestTimerState
import com.forge.app.program.DayPlan
import com.forge.app.program.ExercisePlan

/**
 * UI state for the active workout screen.
 *
 * Warmup state is intentionally in-memory (resets if the VM is recreated). Acceptable
 * for the project's no-cloud, single-user scope — re-checking four boxes on resume
 * is cheaper than a schema bump.
 */
data class DayUiState(
    val dayPlan: DayPlan,
    val displayName: String,
    val isLoading: Boolean = true,
    val sessionId: Long? = null,
    val sessionStartedAt: Long? = null,
    val exercises: List<ExerciseUiState> = emptyList(),
    val warmupChecks: List<Boolean> = List(4) { false },
    val isWarmupComplete: Boolean = false,
    val restTimer: RestTimerState? = null,
    val showTimerControls: Boolean = false,
    val swapPickerForExerciseId: String? = null,
    val summary: SessionSummary? = null,
    val showDiscardConfirm: Boolean = false,
    val isFinished: Boolean = false
) {
    val hasUnsavedWork: Boolean
        get() = exercises.any { it.loggedSets.isNotEmpty() }

    val canSkipWarmup: Boolean
        get() = !isWarmupComplete

    /** Exercise currently targeted by the swap picker, if any. */
    val swapPickerExercise: ExerciseUiState?
        get() = swapPickerForExerciseId?.let { id ->
            exercises.firstOrNull { it.plan.id == id }
        }
}

data class ExerciseUiState(
    val plan: ExercisePlan,
    val loggedExerciseId: Long? = null,
    val loggedSets: List<LoggedSet> = emptyList(),
    val lastSessionPreviewText: String? = null,
    val prefillWeight: String? = null,
    val difficulty: EffortRating? = null,
    val note: String? = null,
    val skipped: Boolean = false,
    val isExpanded: Boolean = false,
    /** True when the user has marked this exercise's session entry as a PR (any set). */
    val wasPr: Boolean = false,
    /** Set ids known to be PRs (subset of [loggedSets].map { it.id }). Used for per-row badges. */
    val prSetIds: Set<Long> = emptySet(),
    /** Active swap for this session only — overrides the static plan's name when non-null. */
    val sessionSwapName: String? = null,
    val sessionSwapUnit: String? = null,
    /** Persistent customization applied to this exercise (from ExerciseCustomization table). */
    val persistentSwapName: String? = null,
    val persistentSwapUnit: String? = null,
    /**
     * Progression-aware weight suggestion (#12/#13). Non-null when the VM determines the user
     * should try a different weight than last time (e.g. +2.5 lb after hitting the rep range top,
     * or -2.5 lb after a Brutal rating). Stored as the raw input string (e.g. "27.5").
     */
    val suggestedWeight: String? = null,
    val suggestionReason: String? = null
) {
    /** Display name preferring session-swap, then persistent-swap, then the static plan name. */
    val effectiveName: String
        get() = sessionSwapName ?: persistentSwapName ?: plan.name
}
