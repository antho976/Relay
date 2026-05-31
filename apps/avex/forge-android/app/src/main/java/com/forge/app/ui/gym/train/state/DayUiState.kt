package com.forge.app.ui.gym.train.state

import androidx.compose.runtime.Immutable
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.timer.RestTimerState
import com.forge.app.program.DayPlan
import com.forge.app.program.ExercisePlan

/** How the current session compares to the previous session on the same exercise. */
enum class VsLastStatus { BEATING, MATCHING, UNDER }

/** Held in state until the user confirms or cancels the suspicious weight (#117). */
data class WeightJumpWarning(
    val exerciseId: String,
    val weightText: String,
    val reps: Int,
    val lastWeightLb: Double,
    val newWeightLb: Double
)

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
    val isFinished: Boolean = false,
    /** Set ID that can be undone within the ~5s window after logging (#46). */
    val undoableSetId: Long? = null,
    /** Exercise whose goal-setter dialog is open. Null when closed (#28). */
    val goalSetterForExerciseId: String? = null,
    /** Pending weight-jump warning (#117): exerciseId + proposed weight text + reps. */
    val pendingWeightJumpWarning: WeightJumpWarning? = null,
    /** Session type (#109). */
    val sessionType: String = "normal",
    /** If true, this session skips streak/trophies/suggestions (#110). */
    val isUntracked: Boolean = false,
    /** Session intensity intent (#123). */
    val sessionIntensity: String = "normal",
    /** Pre-session picker shown once on first open. False after user confirms. */
    val showPreSessionPicker: Boolean = false,
    /** Per-warmup-item reactions (#69). Key = item index, value = true (👍) / false (👎). */
    val warmupReactions: Map<Int, Boolean> = emptyMap(),
    /** True when the add-exercise picker sheet is open (#61). */
    val showAddExercisePicker: Boolean = false,
    /** Exercise whose long-press quick-action menu is open (#25). */
    val quickActionsForExerciseId: String? = null,
    /** Custom warmup list (#120). Null = use dayPlan.warmup. */
    val customWarmupItems: List<String>? = null,
    /** Warmup suggester dialog: exerciseId whose working weight to suggest from (#10). */
    val warmupSuggesterForExerciseId: String? = null,
    /** Plate calculator dialog: exerciseId whose weight to calculate plates for (#11). */
    val plateCalculatorForExerciseId: String? = null
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

    /** First untouched exercise name — shown on the rest timer bubble while resting (#99). */
    val nextUpExerciseName: String?
        get() {
            if (restTimer == null) return null
            return exercises.firstOrNull { !it.skipped && it.loggedSets.isEmpty() }?.effectiveName
        }

    /** "X / Y exercises" progress string for the top bar (#102). Empty while loading. */
    val sessionProgressText: String
        get() {
            if (exercises.isEmpty()) return ""
            val done = exercises.count { it.loggedSets.size >= it.targetSets || it.skipped }
            return "$done / ${exercises.size}"
        }

    /** Remaining planned sets across all non-skipped exercises; used to estimate end time (#103). */
    val remainingSetsCount: Int
        get() = exercises.filter { !it.skipped }
            .sumOf { maxOf(0, it.targetSets - it.loggedSets.size) }
}

@Immutable
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
    val suggestionReason: String? = null,
    /** Historical sets from prior sessions — used to compute the live PR hint in SetInputRow (#100). */
    val priorSets: List<LoggedSet> = emptyList(),
    /** All-time personal best formatted as "X lb × Y" — shown in the card header (#101). */
    val allTimePbText: String? = null,
    /** Raw all-time best weight in lb — used for goal progress computation (#28). */
    val allTimePbLb: Double? = null,
    /** How this session's volume compares to last session for this exercise (#104). */
    val vsLastStatus: VsLastStatus? = null,
    /** User-set goal weight for this exercise in lb (#28). Null if no goal set. */
    val goalWeightLb: Double? = null,
    /** Custom rest duration in seconds for this exercise (#59). Null = use smart defaults. */
    val restTimerOverrideSeconds: Int? = null,
    /** Always-visible pinned cue in the card header (#112). Empty = not shown. */
    val pinnedNote: String = "",
    /** Superset group identifier (#38). Non-null = part of a superset. */
    val supersetGroup: String? = null,
    /**
     * Per-session aggregates for this exercise (newest first, up to ~8 sessions).
     * Fuel for the last-session strip (date · duration · volume) + sparkline.
     */
    val sessionHistory: List<ExerciseSessionPoint> = emptyList(),
    /**
     * Suggested weight delta for the *next* exercise in the session — drives the
     * "+5 ↑" pill on the UP NEXT row. Already includes sign (e.g. "+5", "−2.5").
     */
    val nextSuggestedWeightDelta: String? = null,
    /**
     * Extra sets added beyond the plan this session via "+ ADD A SET" (in-memory,
     * resets if the VM is recreated). Raises [targetSets] so the card doesn't
     * auto-collapse and the counter reflects the new goal.
     */
    val bonusSets: Int = 0
) {
    /** Planned sets plus any session-local bonus sets ("+ ADD A SET"). */
    val targetSets: Int get() = plan.sets + bonusSets

    /** 0f–1f progress toward [goalWeightLb] based on all-time PB. Null if either is absent. */
    val goalProgressFraction: Float?
        get() {
            val pb = allTimePbLb ?: return null
            val goal = goalWeightLb?.takeIf { it > 0 } ?: return null
            return (pb / goal).toFloat().coerceIn(0f, 1f)
        }

    /** Display name preferring session-swap, then persistent-swap, then the static plan name. */
    val effectiveName: String
        get() = sessionSwapName ?: persistentSwapName ?: plan.name
}

/** Slim per-session aggregate for one exercise (for the day-screen ledger strip). */
data class ExerciseSessionPoint(
    val sessionStartedAt: Long,
    val durationMin: Int?,
    val volumeLb: Double,
    val topWeightLb: Double?
)
