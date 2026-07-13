package com.forge.app.ui.gym.train.state

import androidx.compose.runtime.Immutable
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.timer.RestTimerState
import com.forge.app.program.DayPlan
import com.forge.app.program.Equipment
import com.forge.app.program.ExercisePlan
import com.forge.app.program.ExerciseUnit

/** How the current session compares to the previous session on the same exercise. */
enum class VsLastStatus { BEATING, MATCHING, UNDER }

/**
 * Set when this day is opened while a *different* day's workout is still in progress. The app
 * keeps only one active session, so we prompt rather than silently resume the wrong day's sets.
 */
data class CrossDaySessionInfo(val dayKey: String, val dayName: String)

/**
 * The swapped-out exercise the post-swap prompt offers to dislike. [exerciseId] is its
 * ExerciseLibrary id (what [com.forge.app.data.prefs.SettingsRepository.setExerciseDisliked] keys
 * on); [exerciseName] is the display name for the dialog copy.
 */
data class DislikeSwapPrompt(val exerciseId: String, val exerciseName: String)

/**
 * Held in state until the user confirms or cancels the suspicious weight (#117). [lastLabel] /
 * [newLabel] are already formatted in the exercise's own unit (plate counts on PLATES, kg/lb
 * otherwise) so the dialog never shows a raw-lb number the user didn't enter (#1/#10).
 */
data class WeightJumpWarning(
    val exerciseId: String,
    val weightText: String,
    val reps: Int,
    val lastLabel: String,
    val newLabel: String,
    val percent: Int
)

/**
 * UI state for the active workout screen.
 *
 * Warmup state is intentionally in-memory (resets if the VM is recreated). Acceptable
 * for the project's no-cloud, single-user scope: resuming an existing session skips the
 * warmup gate outright (see beginSessionForThisDay), so the lost in-memory state never
 * forces a re-check — cheaper than persisting it with a schema bump.
 */
@Immutable
data class DayUiState(
    val dayPlan: DayPlan,
    val displayName: String,
    val isLoading: Boolean = true,
    val sessionId: Long? = null,
    /** The session's REAL first-start time (for the date pill) — stable across resume sittings. */
    val sessionStartedAt: Long? = null,
    /** Free-text session journal (#111), editable mid-session from the top bar and again at finish. */
    val sessionJournal: String = "",
    /**
     * Anchor for the live ACTIVE-time readout: chosen so (now − anchor) = prior sittings + this
     * sitting so far. Differs from [sessionStartedAt] once a session spans multiple sittings.
     */
    val elapsedAnchorMs: Long? = null,
    val exercises: List<ExerciseUiState> = emptyList(),
    val warmupChecks: List<Boolean> = List(4) { false },
    val isWarmupComplete: Boolean = false,
    val restTimer: RestTimerState? = null,
    val showTimerControls: Boolean = false,
    /** How the running rest duration was derived (engine System 2) — shown in the timer dialog. */
    val restTimerReason: String? = null,
    /**
     * Pre-session fatigue-aware reorder proposal (engine System 3). Non-null only before
     * any set is logged; cleared on apply/dismiss. Applying reorders the in-memory list
     * via the same mechanism as manual reordering — the program itself is never touched.
     */
    val orderingSuggestion: com.forge.app.domain.adapt.Recommendation.SessionOrder? = null,
    val swapPickerForExerciseId: String? = null,
    val summary: SessionSummary? = null,
    val showDiscardConfirm: Boolean = false,
    /** Non-null when opening this day found another day's workout still in progress (resume-later guard). */
    val crossDaySession: CrossDaySessionInfo? = null,
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
    val plateCalculatorForExerciseId: String? = null,
    /** Available equipment — filters the swap picker's candidate pool. Empty = all (program-unlock). */
    val swapAvailableEquipment: Set<Equipment> = emptySet(),
    /** Disliked library ids — excluded from the swap picker, same as generation (program-unlock). */
    val swapDislikedIds: Set<String> = emptySet(),
    /** Curated/frozen exercise pool (Developer's preset) — locks the swap picker to it; null = none. */
    val swapFrozenIds: Set<String>? = null,
    /**
     * Non-null ⇒ show the post-swap prompt offering to dislike the swapped-out exercise. Raised only
     * after a "Make default" (persistent) swap, gated by the swap-dislike-prompt pref + the per-session
     * "not this workout" suppression.
     */
    val dislikeSwapPrompt: DislikeSwapPrompt? = null
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

    /** "Beat the ghost": logged sets that surpassed the same-position set from last session. */
    val ghostBeats: Int
        get() = exercises.sumOf { ex ->
            ex.loggedSets.withIndex().count { (i, s) -> beatsPriorSet(s, ex.priorSets.getOrNull(i)) }
        }

    /** Logged sets that HAVE a last-session counterpart to be judged against (the duel's denominator). */
    val ghostComparable: Int
        get() = exercises.sumOf { ex ->
            ex.loggedSets.indices.count { i -> ex.priorSets.getOrNull(i) != null }
        }
}

/**
 * A logged set "beats" its same-position set from last session when it's heavier, or equally
 * heavy for more reps, or (when lighter, or bodyweight) moves more total volume. The little
 * win that makes every set a duel with your past self.
 */
internal fun beatsPriorSet(cur: LoggedSet, prior: LoggedSet?): Boolean {
    if (prior == null) return false
    val cw = cur.weightLb
    val pw = prior.weightLb
    if (cw == null || pw == null) return cur.reps > prior.reps
    return when {
        cw > pw -> true
        cw == pw -> cur.reps > prior.reps
        else -> cw * cur.reps > pw * prior.reps
    }
}

@Immutable
data class ExerciseUiState(
    val plan: ExercisePlan,
    /**
     * The real exercise this card logs under (#11): the swapped exercise's id when swapped, else the
     * slot id ([plan].id). PR/stats history is read by this, and new sets are written under it, so a
     * swapped exercise's work attributes to the actual exercise — while UI/event matching still uses
     * [plan].id (the slot). Defaults to "" only for never-constructed states; the builder always sets it.
     */
    val effectiveExerciseId: String = "",
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
    /** Set ids known to be PRs (subset of [loggedSets].map { it.id }). Drives the PR count / confetti. */
    val prSetIds: Set<Long> = emptySet(),
    /**
     * The single best PR set of this session (heaviest, then most reps, then latest) — the ONLY row
     * that shows the gold ★ + gold text, so multiple ascending PR sets don't all light up (#3/#4/#7).
     * The highlight moves to the new set when you beat your record.
     */
    val bestPrSetId: Long? = null,
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
    /** Suggested change vs last session's top weight, in lb — drives the UP NEXT delta pill. */
    val suggestedDeltaLb: Double? = null,
    /** Suggested target in raw lb — outcome capture compares it against the first logged set. */
    val suggestedTargetLb: Double? = null,
    /** Bodyweight rep-progression target — "aim for N reps" (CO5). Null for weighted exercises. */
    val suggestedReps: Int? = null,
    val suggestedRepsReason: String? = null,
    /**
     * The PREVIOUS session's sets in performed order — the positional "ghost" comparisons
     * (same-position duels, per-row ghost values) and "last session" labels read these.
     */
    val priorSets: List<LoggedSet> = emptyList(),
    /**
     * All-time per-rep-count max-weight frontier (synthetic sets: only weightLb/reps are real,
     * non-assisted) — the bounded stand-in for full history feeding the live PR hint (#100)
     * and the weight-jump check (#117).
     */
    val priorFrontier: List<LoggedSet> = emptyList(),
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

    /** True when a session or persistent swap is replacing the base exercise — the static
     *  [plan]'s form cue then describes a different movement and must not be shown. */
    val isSwapped: Boolean
        get() = sessionSwapName != null || persistentSwapName != null

    /**
     * The unit the input/set rows should use: the swapped-in exercise's unit when a swap is active
     * (session, then persistent), else the static [plan]'s. Without this a BODYWEIGHT→weighted swap
     * (or vice-versa) kept showing the old weight type — the swap stored the new unit but the card
     * still read [plan].unit. Pairs with [effectiveName] (same session→persistent→base precedence).
     */
    val effectiveUnit: ExerciseUnit
        get() {
            val code = when {
                sessionSwapName != null -> sessionSwapUnit
                persistentSwapName != null -> persistentSwapUnit
                else -> null
            }
            return code?.let { ExerciseUnit.fromCode(it) } ?: plan.unit
        }

    /**
     * Timed-hold exercise (GYMAP-51) — logged as a held DURATION (seconds), not reps: planks, dead
     * hangs, wall sits. Resolved from the EFFECTIVE exercise (swap-aware via [effectiveExerciseId])
     * so swapping in/out of a hold switches the input mode too; custom exercises (not in the library)
     * are never timed. The set-input row then shows a stopwatch + mm:ss instead of a reps field.
     */
    val timed: Boolean
        get() = com.forge.app.program.ExerciseLibrary.byId(effectiveExerciseId)?.timed == true
}

/** Slim per-session aggregate for one exercise (for the day-screen ledger strip). */
data class ExerciseSessionPoint(
    val sessionStartedAt: Long,
    val durationMin: Int?,
    val volumeLb: Double,
    val topWeightLb: Double?
)
