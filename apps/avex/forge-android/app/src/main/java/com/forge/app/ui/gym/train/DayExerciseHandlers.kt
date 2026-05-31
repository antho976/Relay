package com.forge.app.ui.gym.train

import androidx.lifecycle.viewModelScope
import com.forge.app.domain.parser.WeightParser
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.WeightJumpWarning
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

@Suppress("ComplexMethod")
internal fun DayViewModel.handleExerciseEvent(event: DayUiEvent) {
    when (event) {
        is DayUiEvent.ToggleExpanded -> _state.update { s ->
            s.copy(exercises = s.exercises.map {
                if (it.plan.id == event.exerciseId) it.copy(isExpanded = !it.isExpanded) else it
            })
        }
        is DayUiEvent.LogSet -> logSet(event.exerciseId, event.weightText, event.reps)
        is DayUiEvent.LogSameAsLast -> {
            val set = _state.value.exercises.flatMap { it.loggedSets }
                .firstOrNull { it.id == event.setId } ?: return
            logSet(event.exerciseId, set.weightText, set.reps)
        }
        is DayUiEvent.DeleteSet -> viewModelScope.launch {
            val set = findSet(event.setId) ?: return@launch
            val exId = findExerciseIdForSet(event.setId)
            workoutRepo.deleteSet(set)
            if (exId != null) refreshExercise(exId) else refreshExercises()
        }
        is DayUiEvent.EditSet -> viewModelScope.launch {
            if (event.reps <= 0) return@launch
            val exerciseUi = _state.value.exercises
                .firstOrNull { ex -> ex.loggedSets.any { it.id == event.setId } } ?: return@launch
            val plan = dayPlan.exercises.firstOrNull { it.id == exerciseUi.plan.id } ?: return@launch
            val set = exerciseUi.loggedSets.firstOrNull { it.id == event.setId } ?: return@launch
            val lb = WeightParser.parse(event.weightText, plan.unit)
            workoutRepo.updateSet(set.copy(weightText = event.weightText, weightLb = lb, reps = event.reps))
            refreshExercise(exerciseUi.plan.id)
        }
        is DayUiEvent.RateExercise -> viewModelScope.launch {
            val leId = ensureLoggedExercise(event.exerciseId) ?: return@launch
            workoutRepo.setRating(leId, event.rating)
            refreshExercise(event.exerciseId)
        }
        is DayUiEvent.UpdateNote -> viewModelScope.launch {
            val leId = ensureLoggedExercise(event.exerciseId) ?: return@launch
            workoutRepo.setNote(leId, event.note.ifBlank { null })
            refreshExercise(event.exerciseId)
        }
        is DayUiEvent.ToggleSkipped -> viewModelScope.launch {
            val currentUi = _state.value.exercises
                .firstOrNull { it.plan.id == event.exerciseId } ?: return@launch
            val leId = ensureLoggedExercise(event.exerciseId) ?: return@launch
            val newSkipped = !currentUi.skipped
            workoutRepo.setSkipped(leId, newSkipped)
            refreshExercise(event.exerciseId)
            if (newSkipped) {
                val nextEx = _state.value.exercises
                    .dropWhile { it.plan.id != event.exerciseId }.drop(1)
                    .firstOrNull { !it.skipped && it.loggedSets.size < it.targetSets }
                if (nextEx != null) _state.update { s ->
                    s.copy(exercises = s.exercises.map {
                        if (it.plan.id == nextEx.plan.id) it.copy(isExpanded = true) else it
                    })
                }
            }
        }
        is DayUiEvent.OpenGoalSetter -> _state.update { it.copy(goalSetterForExerciseId = event.exerciseId) }
        is DayUiEvent.SetGoal -> viewModelScope.launch {
            goalRepo.setGoal(event.exerciseId, event.targetWeightLb)
            _state.update { it.copy(goalSetterForExerciseId = null) }
            refreshExercise(event.exerciseId)
        }
        is DayUiEvent.ClearGoal -> viewModelScope.launch {
            goalRepo.clearGoal(event.exerciseId)
            _state.update { it.copy(goalSetterForExerciseId = null) }
            refreshExercise(event.exerciseId)
        }
        is DayUiEvent.DismissGoalSetter -> _state.update { it.copy(goalSetterForExerciseId = null) }
        is DayUiEvent.SetExerciseUnit -> viewModelScope.launch {
            val existing = customizationRepo.getSwap(event.exerciseId)
            if (event.unit == null) {
                if (existing?.swappedName?.isBlank() == true) customizationRepo.clearSwap(event.exerciseId)
                existing?.let { customizationRepo.setSwap(it.exerciseId, it.swappedName, "") }
            } else {
                customizationRepo.setSwap(event.exerciseId, existing?.swappedName ?: "", event.unit)
            }
            refreshExercise(event.exerciseId)
        }
        is DayUiEvent.SetRestTimerOverride -> viewModelScope.launch {
            customizationRepo.setRestTimerOverride(event.exerciseId, event.seconds)
            refreshExercise(event.exerciseId)
        }
        is DayUiEvent.ConfirmWeightJump -> {
            val warning = _state.value.pendingWeightJumpWarning ?: return
            _state.update { it.copy(pendingWeightJumpWarning = null) }
            // Bypass the jump check — the user already confirmed; otherwise it re-triggers.
            logSet(warning.exerciseId, warning.weightText, warning.reps, skipJumpCheck = true)
        }
        is DayUiEvent.DismissWeightJump -> _state.update { it.copy(pendingWeightJumpWarning = null) }
        is DayUiEvent.UpdateJournal ->
            _state.value.sessionId?.let { id ->
                viewModelScope.launch { workoutRepo.setJournal(id, event.text) }
            }
        is DayUiEvent.SetPinnedNote -> viewModelScope.launch {
            customizationRepo.setPinnedNote(event.exerciseId, event.note)
            refreshExercise(event.exerciseId)
        }
        is DayUiEvent.ToggleSetDifficultyTag -> viewModelScope.launch {
            val nextTag = when (event.currentTag) { null -> "easy"; "easy" -> "hard"; else -> null }
            workoutRepo.setDifficultyTag(event.setId, nextTag)
            refreshExerciseForSet(event.setId)
        }
        is DayUiEvent.WarmupReaction -> {
            val current = _state.value.warmupReactions.toMutableMap()
            if (current[event.index] == event.thumbsUp) current.remove(event.index)
            else current[event.index] = event.thumbsUp
            _state.update { it.copy(warmupReactions = current) }
        }
        is DayUiEvent.MoveExercise -> {
            val exercises = _state.value.exercises.toMutableList()
            val idx = exercises.indexOfFirst { it.plan.id == event.exerciseId }
            val newIdx = (idx + event.direction).coerceIn(0, exercises.lastIndex)
            if (idx != newIdx) {
                exercises.add(newIdx, exercises.removeAt(idx))
                _state.update { it.copy(exercises = exercises) }
            }
        }
        is DayUiEvent.LongPressExercise -> _state.update { it.copy(quickActionsForExerciseId = event.exerciseId) }
        is DayUiEvent.DismissQuickActions -> _state.update { it.copy(quickActionsForExerciseId = null) }
        is DayUiEvent.OpenAddExercisePicker -> _state.update { it.copy(showAddExercisePicker = true) }
        is DayUiEvent.CloseAddExercisePicker -> _state.update { it.copy(showAddExercisePicker = false) }
        is DayUiEvent.AddUnplannedExercise -> viewModelScope.launch {
            val sessionId = _state.value.sessionId ?: return@launch
            val existing = _state.value.exercises
            if (existing.any { it.plan.id == event.exerciseId }) {
                _state.update { it.copy(showAddExercisePicker = false) }
                return@launch
            }
            workoutRepo.addExerciseToSession(sessionId = sessionId, exerciseId = event.exerciseId, orderIndex = existing.size)
            _state.update { it.copy(showAddExercisePicker = false) }
            refreshExercises()
        }
        is DayUiEvent.ToggleAmrap -> viewModelScope.launch {
            val set = findSet(event.setId) ?: return@launch
            workoutRepo.setAmrap(event.setId, !set.isAmrap)
            refreshExerciseForSet(event.setId)
        }
        is DayUiEvent.ToggleAssisted -> viewModelScope.launch {
            val set = findSet(event.setId) ?: return@launch
            workoutRepo.setAssisted(event.setId, !set.isAssisted)
            refreshExerciseForSet(event.setId)
        }
        is DayUiEvent.ToggleFailure -> viewModelScope.launch {
            val set = findSet(event.setId) ?: return@launch
            workoutRepo.setToFailure(event.setId, !set.toFailure)
            refreshExerciseForSet(event.setId)
        }
        is DayUiEvent.SetSetType -> viewModelScope.launch {
            workoutRepo.setSetType(event.setId, event.type)
            refreshExerciseForSet(event.setId)
        }
        is DayUiEvent.SetDropAnnotation -> viewModelScope.launch {
            workoutRepo.setDropAnnotation(event.setId, event.annotation)
            refreshExerciseForSet(event.setId)
        }
        is DayUiEvent.SetRpe -> viewModelScope.launch {
            workoutRepo.setRpe(event.setId, event.rpe)
            refreshExerciseForSet(event.setId)
        }
        is DayUiEvent.AddBonusSet -> _state.update { s ->
            s.copy(exercises = s.exercises.map {
                if (it.plan.id == event.exerciseId) it.copy(bonusSets = it.bonusSets + 1, isExpanded = true) else it
            })
        }
        is DayUiEvent.SetUseKg -> viewModelScope.launch { settingsRepo.setUseKg(event.useKg) }
        is DayUiEvent.SetSupersetGroup -> viewModelScope.launch {
            val loggedId = _state.value.exercises
                .firstOrNull { it.plan.id == event.exerciseId }?.loggedExerciseId ?: return@launch
            workoutRepo.setSupersetGroup(loggedId, event.group)
            refreshExercise(event.exerciseId)
        }
        is DayUiEvent.ShowWarmupSuggester -> _state.update { it.copy(warmupSuggesterForExerciseId = event.exerciseId) }
        is DayUiEvent.ShowPlateCalculator -> _state.update { it.copy(plateCalculatorForExerciseId = event.exerciseId) }
        is DayUiEvent.DismissTrainingHelper -> _state.update {
            it.copy(warmupSuggesterForExerciseId = null, plateCalculatorForExerciseId = null)
        }
        is DayUiEvent.LogBreak -> {
            val sessionId = _state.value.sessionId ?: return
            viewModelScope.launch { workoutRepo.logBreak(sessionId, event.type) }
        }
        else -> {}
    }
}

internal fun DayViewModel.logSet(exerciseId: String, weightText: String, reps: Int, skipJumpCheck: Boolean = false) {
    if (reps <= 0) return
    viewModelScope.launch {
        val sessionId = _state.value.sessionId ?: return@launch
        val plan = dayPlan.exercises.firstOrNull { it.id == exerciseId } ?: return@launch
        val currentUi = _state.value.exercises.firstOrNull { it.plan.id == exerciseId } ?: return@launch

        val newWeightLb = WeightParser.parse(weightText, plan.unit)
        // Ignore dummy display rows (loggedExerciseId == -1) so placeholder data never
        // triggers the jump warning.
        val lastWeightLb = currentUi.priorSets
            .filter { it.loggedExerciseId != -1L }
            .mapNotNull { it.weightLb }
            .maxOrNull()
        if (!skipJumpCheck && newWeightLb != null && lastWeightLb != null && lastWeightLb > 0 &&
            newWeightLb > lastWeightLb * 1.20
        ) {
            _state.update {
                it.copy(pendingWeightJumpWarning = WeightJumpWarning(exerciseId, weightText, reps, lastWeightLb, newWeightLb))
            }
            return@launch
        }

        // Start the rest timer before the DB write so the bubble + countdown appear the
        // instant you tap — not after the insert and per-exercise rebuild round-trip.
        restTimer.start(computeTimerDuration(plan, currentUi.difficulty, currentUi.restTimerOverrideSeconds))

        val leId = currentUi.loggedExerciseId
            ?: workoutRepo.addExerciseToSession(
                sessionId = sessionId,
                exerciseId = exerciseId,
                orderIndex = dayPlan.exercises.indexOfFirst { it.id == exerciseId },
                swappedName = currentUi.sessionSwapName ?: currentUi.persistentSwapName,
                swappedUnit = currentUi.sessionSwapUnit ?: currentUi.persistentSwapUnit
            )

        workoutRepo.logSet(
            loggedExerciseId = leId,
            setIndex = currentUi.loggedSets.size,
            weightText = weightText,
            weightLb = newWeightLb,
            reps = reps
        )
        refreshExercise(exerciseId)

        val updatedEx = _state.value.exercises.firstOrNull { it.plan.id == exerciseId }
        if (updatedEx != null && updatedEx.loggedSets.size >= updatedEx.targetSets) {
            _state.update { s ->
                s.copy(exercises = s.exercises.map {
                    if (it.plan.id == exerciseId) it.copy(isExpanded = false) else it
                })
            }
        }

        val newSetId = _state.value.exercises
            .firstOrNull { it.plan.id == exerciseId }?.loggedSets?.lastOrNull()?.id
        if (newSetId != null) {
            _state.update { it.copy(undoableSetId = newSetId) }
            undoClearJob?.cancel()
            undoClearJob = viewModelScope.launch {
                delay(5_000)
                _state.update { it.copy(undoableSetId = null) }
            }
        }
    }
}
