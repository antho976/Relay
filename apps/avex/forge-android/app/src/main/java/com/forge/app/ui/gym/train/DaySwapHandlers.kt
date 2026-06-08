package com.forge.app.ui.gym.train

import androidx.lifecycle.viewModelScope
import com.forge.app.ui.gym.train.state.DayUiEvent
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

internal fun DayViewModel.handleSwapEvent(event: DayUiEvent) {
    when (event) {
        is DayUiEvent.OpenSwapPicker -> _state.update { it.copy(swapPickerForExerciseId = event.exerciseId) }
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
        // difficulty, orderIndex, etc. (the old positional rebuild silently dropped them).
        workoutRepo.setSessionSwap(leId, swap.name, swap.unit.code)
        _state.update { it.copy(swapPickerForExerciseId = null) }
        refreshExercises()
    }
}

private fun DayViewModel.applyPersistentSwap(exerciseId: String, swap: com.forge.app.program.ExerciseDef) {
    viewModelScope.launch {
        customizationRepo.setSwap(exerciseId, swap.name, swap.unit.code)
        applySessionSwap(exerciseId, swap)
    }
}

private fun DayViewModel.clearPersistentSwap(exerciseId: String) {
    viewModelScope.launch {
        customizationRepo.clearSwap(exerciseId)
        _state.update { it.copy(swapPickerForExerciseId = null) }
        refreshExercises()
    }
}
