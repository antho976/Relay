package com.forge.app.ui.gym.train

import android.content.Intent
import com.forge.app.service.SessionNotifState
import com.forge.app.service.WorkoutSessionService
import kotlinx.coroutines.flow.update

// Exercise-state refresh/derivation, set lookups, and session-service lifecycle —
// extracted from DayViewModel as extension functions, matching the Day*Handlers /
// DayViewModelBuilders pattern. All access internal DayViewModel members.

internal suspend fun DayViewModel.refreshExercises() {
    val sessionId = _state.value.sessionId ?: return
    val loggedExercises = workoutRepo.loggedExercisesForSession(sessionId)
    val byExerciseId = loggedExercises.associateBy { it.exerciseId }
    val previousExpandedById = _state.value.exercises.associate { it.plan.id to it.isExpanded }
    val previousBonusById = _state.value.exercises.associate { it.plan.id to it.bonusSets }
    val effectivePlans = programCustomRepo.effectivePlanForDay(dayKey)
    val exercises = effectivePlans.mapIndexed { index, plan ->
        buildExerciseUi(
            plan = plan,
            logged = byExerciseId[plan.id],
            expandedDefault = (index == 0),
            expandedOverride = previousExpandedById[plan.id],
            bonusSets = previousBonusById[plan.id] ?: 0
        )
    }
    val annotated = annotateNextExerciseDeltas(exercises)
    _state.update { it.copy(isLoading = false, exercises = annotated) }
}

/**
 * Rebuild only the one exercise the user just touched, instead of re-deriving every
 * exercise in the day. Logging a set on a 6-exercise day was ~40 sequential DB
 * round-trips (≈7 per exercise) re-deriving unchanged data; this makes it ≈7.
 * Falls back to a full [refreshExercises] when the exercise isn't in the list yet.
 */
internal suspend fun DayViewModel.refreshExercise(exerciseId: String) {
    val sessionId = _state.value.sessionId ?: return
    val current = _state.value.exercises
    val idx = current.indexOfFirst { it.plan.id == exerciseId }
    if (idx < 0) { refreshExercises(); return }
    val existing = current[idx]
    val logged = workoutRepo.loggedExercisesForSession(sessionId)
        .firstOrNull { it.exerciseId == exerciseId }
    val rebuilt = buildExerciseUi(
        plan = existing.plan,
        logged = logged,
        expandedDefault = idx == 0,
        expandedOverride = existing.isExpanded,
        bonusSets = existing.bonusSets
    )
    val newList = current.toMutableList().also { it[idx] = rebuilt }
    _state.update { it.copy(isLoading = false, exercises = annotateNextExerciseDeltas(newList)) }
}

/** Resolve the exercise that owns [setId] and rebuild just it (per-set edits). */
internal suspend fun DayViewModel.refreshExerciseForSet(setId: Long) {
    val exId = findExerciseIdForSet(setId)
    if (exId != null) refreshExercise(exId) else refreshExercises()
}

internal fun DayViewModel.findExerciseIdForSet(setId: Long): String? =
    _state.value.exercises.firstOrNull { ex -> ex.loggedSets.any { it.id == setId } }?.plan?.id

internal suspend fun DayViewModel.ensureLoggedExercise(exerciseId: String): Long? {
    val sessionId = _state.value.sessionId ?: return null
    val currentUi = _state.value.exercises.firstOrNull { it.plan.id == exerciseId } ?: return null
    return currentUi.loggedExerciseId
        ?: workoutRepo.addExerciseToSession(
            sessionId = sessionId,
            exerciseId = exerciseId,
            orderIndex = dayPlan.exercises.indexOfFirst { it.id == exerciseId },
            swappedName = currentUi.sessionSwapName ?: currentUi.persistentSwapName,
            swappedUnit = currentUi.sessionSwapUnit ?: currentUi.persistentSwapUnit
        )
}

internal fun DayViewModel.findSet(setId: Long) =
    _state.value.exercises.flatMap { it.loggedSets }.firstOrNull { it.id == setId }

internal fun DayViewModel.startSessionService(dayName: String) {
    bridge.startSession(SessionNotifState(dayName, clock.nowMs()))
    appContext.startForegroundService(Intent(appContext, WorkoutSessionService::class.java))
}

internal fun DayViewModel.stopSessionService() {
    bridge.endSession()
    appContext.stopService(Intent(appContext, WorkoutSessionService::class.java))
}
