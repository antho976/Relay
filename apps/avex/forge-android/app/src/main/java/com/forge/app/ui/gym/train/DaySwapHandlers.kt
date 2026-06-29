package com.forge.app.ui.gym.train

import androidx.lifecycle.viewModelScope
import com.forge.app.program.ExerciseDef
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.DislikeSwapPrompt
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

internal fun DayViewModel.handleSwapEvent(event: DayUiEvent) {
    when (event) {
        is DayUiEvent.OpenSwapPicker -> viewModelScope.launch {
            // Swapping is impossible once sets are logged: re-keying would mis-attribute the performed
            // sets, and a name-only relabel would lie about what they were (#11). Warn instead of opening.
            // Confirm against the DB, not just in-memory state (SM-5): a freshly-resumed VM or an in-flight
            // refresh can show zero sets for a row that already has them, which would open the picker and
            // silently downgrade the swap to a relabel.
            val ui = _state.value.exercises.firstOrNull { it.plan.id == event.exerciseId }
            val hasSets = ui?.loggedSets?.isNotEmpty() == true ||
                ui?.loggedExerciseId?.let { workoutRepo.setsFor(it).isNotEmpty() } == true
            if (hasSets) {
                _messages.trySend("Can't swap after logging sets — delete this exercise's sets first.")
            } else {
                _state.update { it.copy(swapPickerForExerciseId = event.exerciseId) }
            }
        }
        is DayUiEvent.CloseSwapPicker -> _state.update { it.copy(swapPickerForExerciseId = null) }
        is DayUiEvent.PickSwapForSession -> applySessionSwap(event.exerciseId, event.swap)
        is DayUiEvent.PickSwapPersistent -> applyPersistentSwap(event.exerciseId, event.swap)
        is DayUiEvent.ClearPersistentSwap -> clearPersistentSwap(event.exerciseId)

        // Post-swap "dislike the swapped-out exercise?" prompt
        is DayUiEvent.DislikeSwappedExercise -> dislikeSwappedExercise()
        is DayUiEvent.DismissDislikePrompt -> _state.update { it.copy(dislikeSwapPrompt = null) }
        is DayUiEvent.SuppressDislikePromptThisSession -> {
            dislikePromptSuppressedThisSession = true
            _state.update { it.copy(dislikeSwapPrompt = null) }
        }
        is DayUiEvent.NeverAskDislikePrompt -> neverAskDislikePrompt()
        else -> {}
    }
}

private fun DayViewModel.applySessionSwap(exerciseId: String, swap: ExerciseDef) {
    viewModelScope.launch { doSessionSwap(exerciseId, swap) }
}

/** The shared session-swap body — applied by both "Just today" and "Make default" (the latter after
 *  it has persisted the customization). [suspend] so the persistent path can await it before deciding
 *  whether to raise the dislike prompt. */
private suspend fun DayViewModel.doSessionSwap(exerciseId: String, swap: ExerciseDef) {
    val leId = ensureLoggedExercise(exerciseId) ?: return
    // Update only the swap fields via copy — preserves supersetGroup, hitFullTarget, note,
    // difficulty, orderIndex, etc. (the old positional rebuild silently dropped them). swap.id
    // re-keys exercise_id so PRs/stats follow the swapped exercise (#11).
    workoutRepo.setSessionSwap(leId, swap.name, swap.unit.code, swap.id)
    _state.update { it.copy(swapPickerForExerciseId = null) }
    refreshExercises()
}

private fun DayViewModel.applyPersistentSwap(exerciseId: String, swap: ExerciseDef) {
    viewModelScope.launch {
        // Capture the exercise being replaced BEFORE the swap re-keys the card (#11): effectiveExerciseId
        // is its ExerciseLibrary id (what dislikes key on), effectiveName its display name — both needed
        // for the post-swap dislike prompt.
        val ui = _state.value.exercises.firstOrNull { it.plan.id == exerciseId }
        val originalId = ui?.effectiveExerciseId?.takeIf { it.isNotBlank() } ?: exerciseId
        val originalName = ui?.effectiveName ?: originalId

        customizationRepo.setSwap(exerciseId, swap.name, swap.unit.code, swappedExerciseId = swap.id)
        doSessionSwap(exerciseId, swap)
        maybeShowDislikePrompt(originalId, originalName, swap.id)
    }
}

/** Raise the dislike prompt for the swapped-out exercise unless the user has muted it (pref off / "not
 *  this workout") or it'd be pointless (swapped to itself, or it's already disliked). */
private suspend fun DayViewModel.maybeShowDislikePrompt(originalId: String, originalName: String, swapId: String) {
    val show = shouldShowDislikePrompt(
        promptEnabled = settingsRepo.swapDislikePromptEnabled.first(),
        suppressedThisSession = dislikePromptSuppressedThisSession,
        originalId = originalId,
        swapId = swapId,
        alreadyDisliked = originalId in settingsRepo.dislikedExercises.first()
    )
    if (show) _state.update { it.copy(dislikeSwapPrompt = DislikeSwapPrompt(originalId, originalName)) }
}

/** Pure gate for the post-swap dislike prompt — unit-tested in DaySwapDislikePromptTest. */
internal fun shouldShowDislikePrompt(
    promptEnabled: Boolean,
    suppressedThisSession: Boolean,
    originalId: String,
    swapId: String,
    alreadyDisliked: Boolean
): Boolean =
    promptEnabled && !suppressedThisSession && originalId != swapId && !alreadyDisliked

private fun DayViewModel.dislikeSwappedExercise() {
    val prompt = _state.value.dislikeSwapPrompt ?: return
    viewModelScope.launch {
        settingsRepo.setExerciseDisliked(prompt.exerciseId, true)
        _state.update { it.copy(dislikeSwapPrompt = null) }
        _messages.trySend("Won't suggest ${prompt.exerciseName} again.")
    }
}

private fun DayViewModel.neverAskDislikePrompt() {
    viewModelScope.launch {
        settingsRepo.setSwapDislikePromptEnabled(false)
        _state.update { it.copy(dislikeSwapPrompt = null) }
        _messages.trySend("Turned off — re-enable in Settings → Exercise preferences.")
    }
}

private fun DayViewModel.clearPersistentSwap(exerciseId: String) {
    viewModelScope.launch {
        customizationRepo.clearSwap(exerciseId)
        // Applying a swap eagerly created a logged entry; clearing the swap must revert that still-empty
        // entry back to the slot, or the card stays stuck on the swapped exercise (#11).
        _state.value.exercises.firstOrNull { it.plan.id == exerciseId }?.loggedExerciseId
            ?.let { workoutRepo.revertSwapToSlotIfEmpty(it) }
        _state.update { it.copy(swapPickerForExerciseId = null) }
        refreshExercises()
    }
}
