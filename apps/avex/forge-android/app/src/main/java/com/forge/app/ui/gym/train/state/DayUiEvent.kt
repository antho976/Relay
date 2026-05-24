package com.forge.app.ui.gym.train.state

import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.mood.Mood
import com.forge.app.program.Swap

sealed interface DayUiEvent {
    // Exercise card interactions
    data class ToggleExpanded(val exerciseId: String) : DayUiEvent
    data class LogSet(val exerciseId: String, val weightText: String, val reps: Int) : DayUiEvent
    /** Long-press on a logged set row — immediately log another set with the same weight/reps. */
    data class LogSameAsLast(val exerciseId: String, val setId: Long) : DayUiEvent
    data class DeleteSet(val setId: Long) : DayUiEvent
    data class EditSet(val setId: Long, val weightText: String, val reps: Int) : DayUiEvent
    data class RateExercise(val exerciseId: String, val rating: EffortRating) : DayUiEvent
    data class UpdateNote(val exerciseId: String, val note: String) : DayUiEvent
    data class ToggleSkipped(val exerciseId: String) : DayUiEvent

    // Swap picker
    data class OpenSwapPicker(val exerciseId: String) : DayUiEvent
    data object CloseSwapPicker : DayUiEvent
    /** Apply [swap] for this session only — does not change the default exercise. */
    data class PickSwapForSession(val exerciseId: String, val swap: Swap) : DayUiEvent
    /** Apply [swap] for this session AND persist as the new default for this exercise. */
    data class PickSwapPersistent(val exerciseId: String, val swap: Swap) : DayUiEvent
    /** Clear any persistent swap for this exercise (revert to program default). */
    data class ClearPersistentSwap(val exerciseId: String) : DayUiEvent

    // Warmup
    data class ToggleWarmupItem(val index: Int) : DayUiEvent
    data object SkipWarmup : DayUiEvent

    // Rest timer
    data object RestTimerOpen : DayUiEvent
    data object RestTimerClose : DayUiEvent
    data object RestTimerPause : DayUiEvent
    data object RestTimerResume : DayUiEvent
    data object RestTimerReset : DayUiEvent
    data object RestTimerSkip : DayUiEvent
    data class RestTimerAddSeconds(val seconds: Int) : DayUiEvent

    // Session lifecycle
    data object FinishWorkout : DayUiEvent
    /** Dismiss the summary sheet. [mood] and [tags] are saved against the session. */
    data class DismissSummary(val mood: Mood? = null, val tags: List<String> = emptyList()) : DayUiEvent
    data object RequestBack : DayUiEvent
    data object ConfirmDiscard : DayUiEvent
    data object DismissDiscardConfirm : DayUiEvent
    /** Save session immediately and exit without showing the summary sheet (#97). */
    data object SaveAndExit : DayUiEvent
    /** Undo the last logged set within the ~5s window (#46). */
    data object UndoLastSet : DayUiEvent
    // Goal weight (#28)
    data class OpenGoalSetter(val exerciseId: String) : DayUiEvent
    data class SetGoal(val exerciseId: String, val targetWeightLb: Double) : DayUiEvent
    data class ClearGoal(val exerciseId: String) : DayUiEvent
    data object DismissGoalSetter : DayUiEvent
    // Per-exercise unit memory (#70)
    data class SetExerciseUnit(val exerciseId: String, val unit: String?) : DayUiEvent
    // Rest timer override (#59)
    data class SetRestTimerOverride(val exerciseId: String, val seconds: Int?) : DayUiEvent
    // Weight jump warning (#117)
    data object ConfirmWeightJump : DayUiEvent
    data object DismissWeightJump : DayUiEvent
    // Session enrichment (#109 #110 #123)
    data class SetSessionType(val type: String) : DayUiEvent
    data class SetUntracked(val v: Boolean) : DayUiEvent
    data class SetIntensity(val intensity: String) : DayUiEvent
    data object ConfirmPreSessionPicker : DayUiEvent
    // Journal (#111)
    data class UpdateJournal(val text: String) : DayUiEvent
    // Pinned note (#112)
    data class SetPinnedNote(val exerciseId: String, val note: String) : DayUiEvent
    // Per-set difficulty tag (#68)
    data class ToggleSetDifficultyTag(val setId: Long, val currentTag: String?) : DayUiEvent
    // Warmup item reaction (#69): thumbsUp = true for 👍, false for 👎
    data class WarmupReaction(val index: Int, val thumbsUp: Boolean) : DayUiEvent
    // Reorder exercises (#25)
    data class MoveExercise(val exerciseId: String, val direction: Int) : DayUiEvent  // -1 up, +1 down
    data class LongPressExercise(val exerciseId: String) : DayUiEvent
    data object DismissQuickActions : DayUiEvent
    // Quick-add unplanned exercise (#61)
    data object OpenAddExercisePicker : DayUiEvent
    data object CloseAddExercisePicker : DayUiEvent
    data class AddUnplannedExercise(val exerciseId: String) : DayUiEvent


    // Set annotation events (#140, #141, #18, #142, #143)
    data class ToggleAmrap(val setId: Long) : DayUiEvent
    data class ToggleAssisted(val setId: Long) : DayUiEvent
    data class ToggleFailure(val setId: Long) : DayUiEvent
    data class SetSetType(val setId: Long, val type: String?) : DayUiEvent
    data class SetDropAnnotation(val setId: Long, val annotation: String?) : DayUiEvent

    // Superset grouping (#38)
    data class SetSupersetGroup(val exerciseId: String, val group: String?) : DayUiEvent

    // Training helper dialogs (#10, #11)
    data class ShowWarmupSuggester(val exerciseId: String) : DayUiEvent
    data class ShowPlateCalculator(val exerciseId: String) : DayUiEvent
    data object DismissTrainingHelper : DayUiEvent

    // Quick break logging (#139)
    data class LogBreak(val type: String) : DayUiEvent
}
