package com.forge.app.ui.gym.train

import androidx.lifecycle.viewModelScope
import com.forge.app.ui.gym.train.state.DayUiEvent
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

internal fun DayViewModel.handleSwapEvent(event: DayUiEvent) {
    when (event) {
        is DayUiEvent.OpenSwapPicker -> {
            // Swapping is impossible once sets are logged: re-keying would mis-attribute the performed
            // sets, and a name-only relabel would lie about what they were (#11). Warn instead of opening.
            val hasSets = _state.value.exercises
                .firstOrNull { it.plan.id == event.exerciseId }?.loggedSets?.isNotEmpty() == true
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
        else -> {}
    }
}

private fun DayViewModel.applySessionSwap(exerciseId: String, swap: com.forge.app.program.ExerciseDef) {
    viewModelScope.launch {
        val leId = ensureLoggedExercise(exerciseId) ?: return@launch
        // Update only the swap fields via copy — preserves supersetGroup, hitFullTarget, note,
        // difficulty, orderIndex, etc. (the old positional rebuild silently dropped them). swap.id
        // re-keys exercise_id so PRs/stats follow the swapped exercise (#11).
        workoutRepo.setSessionSwap(leId, swap.name, swap.unit.code, swap.id)
        _state.update { it.copy(swapPickerForExerciseId = null) }
        refreshExercises()
    }
}

private fun DayViewModel.applyPersistentSwap(exerciseId: String, swap: com.forge.app.program.ExerciseDef) {
    viewModelScope.launch {
        customizationRepo.setSwap(exerciseId, swap.name, swap.unit.code, swappedExerciseId = swap.id)
        applySessionSwap(exerciseId, swap)
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
