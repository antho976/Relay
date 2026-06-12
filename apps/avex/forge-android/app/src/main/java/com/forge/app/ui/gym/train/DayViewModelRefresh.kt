package com.forge.app.ui.gym.train

import android.content.Intent
import com.forge.app.program.Program
import com.forge.app.service.SessionNotifState
import com.forge.app.service.WorkoutSessionService
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.first
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
    val previousOrderById = _state.value.exercises.mapIndexed { i, e -> e.plan.id to i }.toMap()

    val effectivePlans = programCustomRepo.effectivePlanForDay(dayKey)
    val effectiveIds = effectivePlans.mapTo(mutableSetOf()) { it.id }
    // Logged exercises added mid-session that aren't part of the day's plan (e.g. a lift picked
    // from another day) — render them too, resolved from the library, so they don't silently
    // vanish on refresh (leaving an invisible orphan row).
    val extraPlans = loggedExercises
        .filterNot { it.exerciseId in effectiveIds }
        .mapNotNull { Program.exercise(it.exerciseId) }
    val allPlans = effectivePlans + extraPlans

    // Each card's build is independent DB reads — fan them out concurrently instead of
    // deriving the day one exercise at a time (the dominant cost of opening the screen).
    val plateLb = settingsRepo.plateWeightLb.first()
    val dbMaxLb = settingsRepo.maxDbWeightLb.first()
    val built = coroutineScope {
        allPlans.mapIndexed { index, plan ->
            async {
                buildExerciseUi(
                    plan = plan,
                    logged = byExerciseId[plan.id],
                    expandedDefault = (index == 0),
                    expandedOverride = previousExpandedById[plan.id],
                    plateLb = plateLb,
                    dbMaxLb = dbMaxLb,
                    bonusSets = previousBonusById[plan.id] ?: 0
                )
            }
        }.awaitAll()
    }
    // Preserve any manual reordering (MoveExercise) made this session: items keep their prior
    // relative position; brand-new items fall to the end in plan order.
    val exercises = built.sortedWith(
        compareBy(
            { previousOrderById[it.plan.id] ?: Int.MAX_VALUE },
            { allPlans.indexOfFirst { p -> p.id == it.plan.id } }
        )
    )
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
        plateLb = settingsRepo.plateWeightLb.first(),
        dbMaxLb = settingsRepo.maxDbWeightLb.first(),
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
            // Order index from the rendered list, not the static day plan (which is -1 for
            // custom/cross-day exercises that aren't in dayPlan.exercises).
            orderIndex = _state.value.exercises.indexOfFirst { it.plan.id == exerciseId },
            swappedName = currentUi.sessionSwapName ?: currentUi.persistentSwapName,
            swappedUnit = currentUi.sessionSwapUnit ?: currentUi.persistentSwapUnit
        )
}

internal fun DayViewModel.findSet(setId: Long) =
    _state.value.exercises.flatMap { it.loggedSets }.firstOrNull { it.id == setId }

/**
 * Pre-session ordering proposal (engine System 3). Computed once after the first exercise
 * build — only for a fresh session (no sets, nothing skipped) and only when this day's
 * suggestion isn't inside its dismissal cooldown.
 */
internal suspend fun DayViewModel.computeOrderingSuggestion() {
    val exercises = _state.value.exercises
    if (exercises.size < 3 || exercises.any { it.loggedSets.isNotEmpty() || it.skipped }) return
    val items = exercises.map { ex ->
        com.forge.app.domain.adapt.OrderingAdvisor.OrderingItem(
            exerciseId = ex.plan.id,
            muscle = ex.plan.muscle,
            isCompound = com.forge.app.program.SessionEstimate.isCompound(ex.plan),
            supersetGroup = ex.supersetGroup
        )
    }
    val priority = settingsRepo.priorityMuscles.first()
        .mapNotNull { com.forge.app.program.MuscleGroup.fromCode(it) }.toSet()
    val suggestion = com.forge.app.domain.adapt.OrderingAdvisor.suggestOrder(dayKey, items, priority)
        ?.takeIf { it.id !in adaptationRepo.mutedAdviceIds() }
    if (suggestion != null) _state.update { it.copy(orderingSuggestion = suggestion) }
}

internal fun DayViewModel.startSessionService(dayName: String) {
    bridge.startSession(SessionNotifState(dayName, clock.nowMs()))
    appContext.startForegroundService(Intent(appContext, WorkoutSessionService::class.java))
}

internal fun DayViewModel.stopSessionService() {
    bridge.endSession()
    appContext.stopService(Intent(appContext, WorkoutSessionService::class.java))
}
