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

    // Session lifecycle
    data object FinishWorkout : DayUiEvent
    /** Dismiss the summary sheet. [mood] is recorded against the session if non-null. */
    data class DismissSummary(val mood: Mood? = null) : DayUiEvent
    data object RequestBack : DayUiEvent
    data object ConfirmDiscard : DayUiEvent
    data object DismissDiscardConfirm : DayUiEvent
}
