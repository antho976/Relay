package com.forge.app.ui.gym.train

import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.volume.VolumeCalculator
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.ExerciseHighlight
import com.forge.app.ui.gym.train.state.SessionSummary
import com.forge.app.ui.gym.train.state.UnlockedTrophyHighlight
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

internal fun DayViewModel.handleSessionEvent(event: DayUiEvent) {
    when (event) {
        is DayUiEvent.FinishWorkout -> finishWorkout()
        is DayUiEvent.DismissSummary -> dismissSummary(event.mood, event.tags)
        is DayUiEvent.RequestBack -> requestBack()
        is DayUiEvent.ConfirmDiscard -> discardAndExit()
        is DayUiEvent.DismissDiscardConfirm -> _state.update { it.copy(showDiscardConfirm = false) }
        is DayUiEvent.SaveAndExit -> saveAndExit()
        is DayUiEvent.UndoLastSet -> {
            val setId = _state.value.undoableSetId ?: return
            undoClearJob?.cancel()
            _state.update { it.copy(undoableSetId = null) }
            viewModelScope.launch {
                val set = findSet(setId) ?: return@launch
                workoutRepo.deleteSet(set)
                refreshExercises()
            }
        }
        is DayUiEvent.SetSessionType -> {
            _state.update { it.copy(sessionType = event.type) }
            _state.value.sessionId?.let { id ->
                viewModelScope.launch { workoutRepo.setSessionType(id, event.type) }
            }
        }
        is DayUiEvent.SetUntracked -> {
            _state.update { it.copy(isUntracked = event.v) }
            _state.value.sessionId?.let { id ->
                viewModelScope.launch { workoutRepo.setUntracked(id, event.v) }
            }
        }
        is DayUiEvent.SetIntensity -> {
            _state.update { it.copy(sessionIntensity = event.intensity) }
            _state.value.sessionId?.let { id ->
                viewModelScope.launch { workoutRepo.setIntensity(id, event.intensity) }
            }
        }
        is DayUiEvent.ConfirmPreSessionPicker -> _state.update { it.copy(showPreSessionPicker = false) }
        else -> {}
    }
}

private fun DayViewModel.finishWorkout() {
    viewModelScope.launch {
        val sessionId = _state.value.sessionId ?: return@launch
        val startedAt = _state.value.sessionStartedAt ?: clock.nowMs()
        val allSets = _state.value.exercises.flatMap { it.loggedSets }
        val totalVolumeLb = VolumeCalculator.sessionVolumeLb(allSets)
        val prCount = _state.value.exercises.count { it.wasPr }

        workoutRepo.finishSession(sessionId, totalVolumeLb, prCount, allSets.size)
        restTimer.stop()
        stopSessionService()

        val newlyUnlocked = trophyRepo.evaluateAndUnlockNew().map { t ->
            UnlockedTrophyHighlight(id = t.id, name = t.name, description = t.description, icon = t.icon)
        }

        val prevSession = workoutRepo.previousSessionForDay(dayKey, sessionId)
        val vsLastVolumeDelta = prevSession?.totalVolumeLb?.let { totalVolumeLb - it }
        val vsLastSetsDelta = prevSession?.setCount?.let { allSets.size - it }
            ?.takeIf { prevSession.setCount > 0 }
        val bestPrevVolume = workoutRepo.bestPreviousVolumeForDay(dayKey, sessionId) ?: 0.0
        val isBestSession = prevSession != null && totalVolumeLb > bestPrevVolume
        val durationMin = ((clock.nowMs() - startedAt) / 60_000).toInt().coerceAtLeast(0)

        val setsPerMin = if (durationMin > 0) allSets.size.toDouble() / durationMin else 0.0
        val volumePerMin = if (durationMin > 0) totalVolumeLb / durationMin else 0.0
        val densityScore = if (durationMin > 0) totalVolumeLb / durationMin else null
        val avgRestSeconds = computeAvgRestSeconds(workoutRepo.allSetsForSession(sessionId))

        val exercises = _state.value.exercises
        val plannedTotal = exercises.filter { !it.skipped }.sumOf { it.plan.sets }
        val loggedNonSkipped = exercises.filter { !it.skipped }.sumOf { it.loggedSets.size }
        val honestyPct = if (plannedTotal > 0)
            ((loggedNonSkipped.toDouble() / plannedTotal) * 100).toInt().coerceIn(0, 100) else null

        val summary = SessionSummary(
            displayName = _state.value.displayName,
            dayWord = dayPlan.word,
            durationMinutes = durationMin,
            totalVolumeLb = totalVolumeLb,
            prCount = prCount,
            setCount = allSets.size,
            exercisesLogged = exercises.count { it.loggedSets.isNotEmpty() && !it.skipped },
            exercisesSkipped = exercises.count { it.skipped },
            highlights = exercises
                .filter { it.loggedSets.isNotEmpty() || it.skipped }
                .map { ex ->
                    ExerciseHighlight(
                        exerciseName = ex.effectiveName,
                        setsLogged = ex.loggedSets.size,
                        volumeLb = VolumeCalculator.sessionVolumeLb(ex.loggedSets),
                        isPr = ex.wasPr
                    )
                },
            unlockedTrophies = newlyUnlocked,
            vsLastVolumeDelta = vsLastVolumeDelta,
            vsLastSetsDelta = vsLastSetsDelta,
            isBestSession = isBestSession,
            setsPerMin = setsPerMin,
            volumePerMin = volumePerMin,
            densityScore = densityScore,
            avgRestSeconds = avgRestSeconds,
            honestyPct = honestyPct
        )
        _state.update { it.copy(isFinished = true, summary = summary) }
    }
}

private fun DayViewModel.saveAndExit() {
    viewModelScope.launch {
        val sessionId = _state.value.sessionId ?: run {
            _navigation.send(DayNavigationEffect.PopBack)
            return@launch
        }
        val allSets = _state.value.exercises.flatMap { it.loggedSets }
        workoutRepo.finishSession(
            sessionId = sessionId,
            totalVolumeLb = VolumeCalculator.sessionVolumeLb(allSets),
            prCount = _state.value.exercises.count { it.wasPr },
            setCount = allSets.size
        )
        restTimer.stop()
        stopSessionService()
        trophyRepo.evaluateAndUnlockNew()
        _state.update { it.copy(isFinished = true) }
        _navigation.send(DayNavigationEffect.PopBack)
    }
}

private fun DayViewModel.dismissSummary(mood: com.forge.app.domain.mood.Mood?, tags: List<String>) {
    viewModelScope.launch {
        val sessionId = _state.value.sessionId
        if (sessionId != null) {
            if (mood != null) workoutRepo.recordMood(sessionId, dayKey, mood.code)
            if (tags.isNotEmpty()) workoutRepo.setSessionTags(sessionId, tags)
        }
        _state.update { it.copy(summary = null) }
        _navigation.send(DayNavigationEffect.PopBack)
    }
}

private fun DayViewModel.requestBack() {
    if (_state.value.hasUnsavedWork) {
        _state.update { it.copy(showDiscardConfirm = true) }
    } else {
        viewModelScope.launch {
            val sessionId = _state.value.sessionId
            if (sessionId != null) workoutRepo.discardSession(sessionId)
            restTimer.stop()
            stopSessionService()
            _navigation.send(DayNavigationEffect.PopBack)
        }
    }
}

private fun DayViewModel.discardAndExit() {
    viewModelScope.launch {
        val sessionId = _state.value.sessionId ?: return@launch
        workoutRepo.discardSession(sessionId)
        restTimer.stop()
        stopSessionService()
        _state.update { it.copy(showDiscardConfirm = false) }
        _navigation.send(DayNavigationEffect.PopBack)
    }
}

private fun computeAvgRestSeconds(sets: List<LoggedSet>): Int? {
    val gaps = sets
        .groupBy { it.loggedExerciseId }
        .values
        .flatMap { exerciseSets ->
            exerciseSets.sortedBy { it.completedAt }
                .zipWithNext { a, b -> (b.completedAt - a.completedAt) / 1000L }
                .filter { it in 5..600 }
        }
    return if (gaps.isEmpty()) null else gaps.average().toInt()
}
