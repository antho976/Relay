package com.forge.app.ui.gym.train

import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.volume.VolumeCalculator
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.ExerciseHighlight
import com.forge.app.ui.gym.train.state.SessionSummary
import com.forge.app.ui.gym.train.state.UnlockedTrophyHighlight
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

internal fun DayViewModel.handleSessionEvent(event: DayUiEvent) {
    when (event) {
        is DayUiEvent.FinishWorkout -> finishWorkout()
        is DayUiEvent.DismissSummary -> dismissSummary(event.mood, event.tags, event.journal)
        is DayUiEvent.RequestBack -> requestBack()
        is DayUiEvent.ConfirmDiscard -> discardAndExit()
        is DayUiEvent.DismissDiscardConfirm -> _state.update { it.copy(showDiscardConfirm = false) }
        is DayUiEvent.LeaveAndResume -> leaveAndResume()
        is DayUiEvent.CrossDayDiscardAndStart -> crossDayDiscardAndStart()
        is DayUiEvent.CrossDayGoBack -> crossDayGoBack()
        is DayUiEvent.SaveAndExit -> saveAndExit()
        is DayUiEvent.UndoLastSet -> {
            val setId = _state.value.undoableSetId ?: return
            undoClearJob?.cancel()
            _state.update { it.copy(undoableSetId = null) }
            viewModelScope.launch {
                val set = findSet(setId) ?: return@launch
                val exId = findExerciseIdForSet(setId)
                workoutRepo.deleteSet(set)
                if (exId != null) refreshExercise(exId) else refreshExercises()
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
        is DayUiEvent.ApplyOrderingSuggestion -> applyOrderingSuggestion()
        is DayUiEvent.DismissOrderingSuggestion -> {
            val suggestion = _state.value.orderingSuggestion
            _state.update { it.copy(orderingSuggestion = null) }
            // Logged so the engine mutes this day's suggestion for its cooldown window.
            if (suggestion != null) viewModelScope.launch { adaptationRepo.logAdviceDismissed(suggestion.id) }
        }
        else -> {}
    }
}

/** Apply the engine's suggested order via the same in-memory mechanism as manual reordering. */
private fun DayViewModel.applyOrderingSuggestion() {
    val suggestion = _state.value.orderingSuggestion ?: return
    val byId = _state.value.exercises.associateBy { it.plan.id }
    val suggested = suggestion.orderedExerciseIds.toSet()
    val reordered = suggestion.orderedExerciseIds.mapNotNull { byId[it] } +
        _state.value.exercises.filter { it.plan.id !in suggested }
    _state.update { it.copy(exercises = annotateNextExerciseDeltas(reordered), orderingSuggestion = null) }
}

private fun DayViewModel.finishWorkout() {
    viewModelScope.launch {
        val sessionId = _state.value.sessionId ?: return@launch
        val allSets = _state.value.exercises.flatMap { it.loggedSets }
        val totalVolumeLb = VolumeCalculator.sessionVolumeLb(allSets)
        val prCount = _state.value.exercises.count { it.wasPr }

        // finishSession closes the open sitting and returns total ACTIVE seconds (summed across
        // sittings) — the real duration, not wall-clock from the first start.
        val activeSeconds = workoutRepo.finishSession(sessionId, totalVolumeLb, prCount, allSets.size)
        // First-touch onboarding cards hide for good once any workout is completed.
        settingsRepo.setFirstWorkoutDone()
        // An interval left open didn't lead to another set — drop it, don't write it.
        openRestEvent = null
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
        val durationMin = (activeSeconds / 60).coerceAtLeast(0)

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
            honestyPct = honestyPct,
            ghostBeats = _state.value.ghostBeats,
            ghostComparable = _state.value.ghostComparable,
            coachOpinion = com.forge.app.domain.coach.SessionOpinion.of(
                setCount = allSets.size,
                prCount = prCount,
                ghostBeats = _state.value.ghostBeats,
                ghostComparable = _state.value.ghostComparable,
                vsLastVolumeDelta = vsLastVolumeDelta,
                isBestSession = isBestSession,
                honestyPct = honestyPct
            ),
            initialJournal = _state.value.sessionJournal
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
        openRestEvent = null
        restTimer.stop()
        stopSessionService()
        trophyRepo.evaluateAndUnlockNew()
        _state.update { it.copy(isFinished = true) }
        _navigation.send(DayNavigationEffect.PopBack)
    }
}

private fun DayViewModel.dismissSummary(mood: com.forge.app.domain.mood.Mood?, tags: List<String>, journal: String) {
    viewModelScope.launch {
        val sessionId = _state.value.sessionId
        if (sessionId != null) {
            if (mood != null) workoutRepo.recordMood(sessionId, dayKey, mood.code)
            if (tags.isNotEmpty()) workoutRepo.setSessionTags(sessionId, tags)
            // Persist unconditionally (including an empty string): the field is pre-seeded with any
            // mid-session note, so a blank value here means the user deliberately cleared it (#111).
            workoutRepo.setJournal(sessionId, journal)
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
        openRestEvent = null
        restTimer.stop()
        stopSessionService()
        _state.update { it.copy(showDiscardConfirm = false) }
        _navigation.send(DayNavigationEffect.PopBack)
    }
}

/**
 * Leave the screen but keep the session ACTIVE — not finished, not discarded. Logged sets are
 * already persisted, so reopening this day resumes the workout exactly where it was left.
 */
private fun DayViewModel.leaveAndResume() {
    viewModelScope.launch {
        // Close THIS sitting's segment so its active time is banked; reopening the day starts a
        // fresh segment and the live readout resumes from the running total.
        _state.value.sessionId?.let { workoutRepo.closeOpenSegments(it, clock.nowMs()) }
        restTimer.stop()
        stopSessionService()
        _state.update { it.copy(showDiscardConfirm = false) }
        _navigation.send(DayNavigationEffect.PopBack)
    }
}

/** Cross-day prompt → discard the other in-progress workout, then start this day fresh. */
private fun DayViewModel.crossDayDiscardAndStart() {
    viewModelScope.launch {
        workoutRepo.activeSession()?.let { workoutRepo.discardSession(it.id) }
        _state.update { it.copy(crossDaySession = null) }
        beginSessionForThisDay()
    }
}

/** Cross-day prompt → leave so the other in-progress workout can be resumed from its own day. */
private fun DayViewModel.crossDayGoBack() {
    viewModelScope.launch { _navigation.send(DayNavigationEffect.PopBack) }
}

/** Start a new session for this day (or resume this day's existing one) and hydrate the screen. */
internal suspend fun DayViewModel.beginSessionForThisDay() {
    val started = workoutRepo.startOrResumeSession(dayKey)
    val sessionId = started.session.id
    // Resuming a session that already has logged work — you're mid-workout, so skip the warmup gate
    // (the in-memory warmup state was lost when the screen was recreated).
    val resuming = started.hasLoggedWork
    val nameOverride = customizationRepo.getDayName(dayKey)
    val resolvedName = nameOverride?.customName ?: dayPlan.defaultName
    val disabledUntilMs = settingsRepo.warmupDisabledUntilMs.firstOrNull() ?: 0L
    val warmupAutoSkipped = clock.nowMs() < disabledUntilMs

    // ── Per-sitting active timing ──────────────────────────────────────────────
    // Close any segment a prior sitting left open (process death), then open THIS sitting's
    // segment. The live elapsed = prior closed sittings + this one, anchored so (now − anchor)
    // reads the running total. The date pill still uses the session's REAL first-start.
    // A just-created session can't have segments yet — skip those queries (screen-open latency).
    val now = clock.nowMs()
    val priorActiveMs = if (started.created) {
        workoutRepo.openSessionSegment(sessionId, now)
        0L
    } else {
        workoutRepo.closeDanglingSegments(sessionId)
        workoutRepo.openSessionSegment(sessionId, now)
        workoutRepo.closedSegmentMs(sessionId)
    }

    _state.update {
        it.copy(
            sessionId = sessionId,
            sessionStartedAt = started.session.startedAt,
            sessionJournal = started.session.journal,
            elapsedAnchorMs = now - priorActiveMs,
            displayName = resolvedName,
            isWarmupComplete = it.isWarmupComplete || skipWarmup || warmupAutoSkipped || resuming
        )
    }
    refreshExercises()
    computeOrderingSuggestion()
    startSessionService(resolvedName)
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
