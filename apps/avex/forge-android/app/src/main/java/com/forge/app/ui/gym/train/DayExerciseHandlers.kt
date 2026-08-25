package com.forge.app.ui.gym.train

import androidx.lifecycle.viewModelScope
import com.forge.app.domain.parser.WeightParser
import com.forge.app.domain.units.formatWeight
import com.forge.app.program.ExerciseUnit
import com.forge.app.ui.gym.train.components.formatPlateCount
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.WeightJumpWarning
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.withContext

@Suppress("ComplexMethod")
internal fun DayViewModel.handleExerciseEvent(event: DayUiEvent) {
    when (event) {
        is DayUiEvent.ToggleExpanded -> _state.update { s ->
            s.copy(exercises = s.exercises.map {
                if (it.plan.id == event.exerciseId) it.copy(isExpanded = !it.isExpanded) else it
            })
        }
        is DayUiEvent.LogSet -> logSet(event.exerciseId, event.weightText, event.reps, durationSeconds = event.durationSeconds)
        is DayUiEvent.LogSameAsLast -> {
            val set = _state.value.exercises.flatMap { it.loggedSets }
                .firstOrNull { it.id == event.setId } ?: return
            // Re-log carries the hold duration too, so repeating a timed set isn't logged as an empty rep set.
            logSet(event.exerciseId, set.weightText, set.reps, durationSeconds = set.durationSeconds)
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
            // Resolve the plan from the rendered exercise (works for custom/cross-day exercises,
            // which aren't in the static dayPlan.exercises).
            val plan = exerciseUi.plan
            val set = exerciseUi.loggedSets.firstOrNull { it.id == event.setId } ?: return@launch
            val lb = WeightParser.parse(event.weightText, plan.unit, settingsRepo.plateWeightLb.first())
            workoutRepo.updateSet(set.copy(weightText = event.weightText, weightLb = lb, reps = event.reps))
            refreshExercise(exerciseUi.plan.id)
        }
        is DayUiEvent.RateExercise -> viewModelScope.launch {
            val leId = ensureLoggedExercise(event.exerciseId) ?: return@launch
            workoutRepo.setRating(leId, event.rating)
            refreshExercise(event.exerciseId)
        }
        is DayUiEvent.UpdateNote -> viewModelScope.launch {
            // The note also commits from NoteField's onDispose, which fires while the screen is
            // tearing down — the moment viewModelScope is about to be cancelled. Without
            // NonCancellable the write is dropped mid-flight, losing exactly the last-moment edit
            // the dispose commit exists to save. The refresh afterwards stays cancellable: it only
            // updates UI state that is going away anyway.
            withContext(NonCancellable) {
                val leId = ensureLoggedExercise(event.exerciseId) ?: return@withContext
                workoutRepo.setNote(leId, event.note.ifBlank { null })
            }
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
                    .firstOrNull { !it.isComplete }
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
        is DayUiEvent.UpdateJournal -> {
            // Keep the in-memory journal in sync so the top-bar editor and the finish sheet
            // both seed from the same live text, then persist.
            _state.update { it.copy(sessionJournal = event.text) }
            _state.value.sessionId?.let { id ->
                viewModelScope.launch { workoutRepo.setJournal(id, event.text) }
            }
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
        // Declare the exercise done early — collapse the card and file it under DONE (mirrors the
        // auto-collapse when the final target set is logged). Navigation is the caller's job.
        // finishedEarly never reaches Room, so the wrist mirror can't see it — the WearFocusHolder
        // mark lets the watch's current-slot pick scan past this slot instead of pinning to it.
        is DayUiEvent.FinishExerciseEarly -> {
            _state.value.sessionId?.let { wearFocusHolder.markEarlyDone(it, event.exerciseId) }
            _state.update { s ->
                s.copy(exercises = s.exercises.map {
                    if (it.plan.id == event.exerciseId) it.copy(finishedEarly = true, isExpanded = false) else it
                })
            }
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

internal fun DayViewModel.logSet(
    exerciseId: String,
    weightText: String,
    reps: Int,
    durationSeconds: Int? = null,
    skipJumpCheck: Boolean = false
) {
    // A timed-hold set (GYMAP-51) carries a duration and reps = 0; a rep set needs reps > 0.
    if (reps <= 0 && durationSeconds == null) return
    viewModelScope.launch {
        val sessionId = _state.value.sessionId ?: return@launch
        val currentUi = _state.value.exercises.firstOrNull { it.plan.id == exerciseId } ?: return@launch
        // Plan comes from the rendered exercise, so custom/cross-day exercises log correctly
        // (the static dayPlan.exercises doesn't contain them).
        val plan = currentUi.plan
        // The real exercise this slot logs under — the swapped exercise when swapped (#11).
        val effectiveExerciseId = currentUi.effectiveExerciseId.ifBlank { exerciseId }

        val plateLb = settingsRepo.plateWeightLb.first()
        val newWeightLb = WeightParser.parse(weightText, plan.unit, plateLb)
        // Compare against the all-time max from the frontier (never contains dummy display
        // rows) so the warning still means "well above anything you've ever done".
        val lastWeightLb = currentUi.priorFrontier
            .mapNotNull { it.weightLb }
            .maxOrNull()
        if (!skipJumpCheck && newWeightLb != null && lastWeightLb != null && lastWeightLb > 0) {
            val isPlates = plan.unit == ExerciseUnit.PLATES
            // Plate machines step one plate at a time, so a single-plate bump reads as a huge % but
            // is normal progression — only flag a 2+ plate jump. Free weights keep the 20% rule (#1/#10).
            val bigJump = if (isPlates) (newWeightLb - lastWeightLb) / plateLb >= 1.5
            else newWeightLb > lastWeightLb * 1.20
            if (bigJump) {
                val weightUnit = settingsRepo.weightUnit.first()
                val lastLabel = if (isPlates) formatPlates(lastWeightLb, plateLb) else formatWeight(lastWeightLb, weightUnit)
                val newLabel = if (isPlates) formatPlates(newWeightLb, plateLb) else formatWeight(newWeightLb, weightUnit)
                val percent = ((newWeightLb / lastWeightLb - 1) * 100).toInt()
                _state.update {
                    it.copy(pendingWeightJumpWarning = WeightJumpWarning(exerciseId, weightText, reps, lastLabel, newLabel, percent))
                }
                return@launch
            }
        }

        // This set ends the previous rest interval (#82) — stamp the moment now, but write the
        // event only after the timer is already on screen so the insert never delays the bubble.
        val restEndedAtMs = clock.nowMs()

        // Start the rest timer before the DB write so the bubble + countdown appear the
        // instant you tap — not after the insert and per-exercise rebuild round-trip.
        // Rest follows the exercise actually performed (#11): a swapped slot uses the swapped exercise's
        // movement profile (compound/isolation + rep heaviness), so a cross-type swap rests correctly and
        // the realized-rest sample (keyed below by effectiveExerciseId) tunes the matching role. Falls back
        // to the slot plan when the id can't be resolved.
        val restPlan = com.forge.app.program.Program.exercise(effectiveExerciseId) ?: plan
        val rest = computeRestPrescription(restPlan, currentUi.difficulty, currentUi.restTimerOverrideSeconds)
        restTimer.start(rest.seconds)
        // Push the started timer into UI state synchronously so it's visible before refreshExercise
        // re-renders — don't wait for the collector coroutine to forward the first emission.
        _state.update { it.copy(restTimer = restTimer.state.value, restTimerReason = rest.reason) }

        closeOpenRestEvent(sessionId, restEndedAtMs)
        openRestEvent = OpenRestEvent(
            // Key the rest sample on the performed exercise (#11) so RestAdvisor.samples re-derives the
            // role from the swapped exercise's plan, matching the prescription above — not the slot's.
            exerciseId = effectiveExerciseId,
            setIndex = currentUi.loggedSets.size,
            plannedSeconds = rest.seconds,
            startedAtMs = restEndedAtMs
        )

        // Was an inline copy of ensureLoggedExercise's body, and the copy had no guard: two rapid
        // taps both read a stale null from UI state and both inserted a row for the same slot.
        val leId = ensureLoggedExercise(exerciseId) ?: return@launch

        workoutRepo.logSet(
            loggedExerciseId = leId,
            setIndex = currentUi.loggedSets.size,
            weightText = weightText,
            weightLb = newWeightLb,
            reps = reps,
            durationSeconds = durationSeconds
        )

        // First set of this exercise while a suggestion chip was showing → record suggestion vs
        // reality for the coach's step calibration (auto-coach Phase 2). Write-only, no reads.
        val suggestedLb = currentUi.suggestedTargetLb
        if (currentUi.loggedSets.isEmpty() && newWeightLb != null && suggestedLb != null) {
            workoutRepo.recordSuggestionOutcome(
                exerciseId = effectiveExerciseId,
                unitCode = plan.unit.code,
                suggestedLb = suggestedLb,
                takenLb = newWeightLb,
                reps = reps,
                rangeText = plan.reps
            )
        }
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

/** "4 plates" / "2.5 plates" — a plate-loaded weight shown as its plate count for the jump warning. */
private fun formatPlates(lb: Double, plateLb: Double): String {
    val plates = lb / plateLb
    return "${formatPlateCount(plates)} plate${if (plates == 1.0) "" else "s"}"
}
