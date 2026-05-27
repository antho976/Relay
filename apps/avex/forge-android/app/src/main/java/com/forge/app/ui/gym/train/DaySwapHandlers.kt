package com.forge.app.ui.gym.train

import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.LoggedExercise
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

private fun DayViewModel.applySessionSwap(exerciseId: String, swap: com.forge.app.program.Swap) {
    viewModelScope.launch {
        val leId = ensureLoggedExercise(exerciseId) ?: return@launch
        val current = _state.value.exercises.firstOrNull { it.plan.id == exerciseId } ?: return@launch
        workoutRepo.updateExercise(
            LoggedExercise(
                id = leId,
                sessionId = _state.value.sessionId!!,
                exerciseId = exerciseId,
                orderIndex = dayPlan.exercises.indexOfFirst { it.id == exerciseId },
                swappedName = swap.name,
                swappedUnit = swap.unit.code,
                difficulty = current.difficulty,
                hitFullTarget = false,
                wasPr = current.wasPr,
                note = current.note,
                skipped = current.skipped
            )
        )
        _state.update { it.copy(swapPickerForExerciseId = null) }
        refreshExercises()
    }
}

private fun DayViewModel.applyPersistentSwap(exerciseId: String, swap: com.forge.app.program.Swap) {
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
