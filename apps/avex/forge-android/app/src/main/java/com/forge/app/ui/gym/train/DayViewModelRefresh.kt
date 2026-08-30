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
import kotlinx.coroutines.sync.withLock

// Exercise-state refresh/derivation, set lookups, and session-service lifecycle —
// extracted from DayViewModel as extension functions, matching the Day*Handlers /
// DayViewModelBuilders pattern. All access internal DayViewModel members.

internal suspend fun DayViewModel.refreshExercises() {
    val sessionId = _state.value.sessionId ?: return
    val loggedExercises = workoutRepo.loggedExercisesForSession(sessionId)
    // Match logged entries to plan slots by their SLOT id (swap-aware): a swapped entry's exercise_id
    // is the swapped exercise, but slot_id still points at the slot it fills (#11), so the slot stays
    // in place and history continuity for the slot's position is preserved.
    //
    // A slot should hold exactly one entry, but the schema doesn't enforce it (a cross-day add or a
    // stale pre-v22 row can collide on effectiveSlotId). On collision keep the entry with the MOST
    // logged sets — so real work never hides behind an empty swap stub — rather than letting list
    // order silently decide (associateBy was last-wins). The set-count query only runs on collisions.
    val grouped = loggedExercises.groupBy { it.effectiveSlotId }
    val bySlotId = HashMap<String, com.forge.app.data.db.entities.LoggedExercise>(grouped.size)
    for ((slot, entries) in grouped) {
        bySlotId[slot] =
            if (entries.size == 1) entries[0]
            else entries.maxByOrNull { workoutRepo.setsFor(it.id).size } ?: entries.last()
    }
    val previousExpandedById = _state.value.exercises.associate { it.plan.id to it.isExpanded }
    val previousBonusById = _state.value.exercises.associate { it.plan.id to it.bonusSets }
    val previousFinishedEarlyById = _state.value.exercises.associate { it.plan.id to it.finishedEarly }
    val previousOrderById = _state.value.exercises.mapIndexed { i, e -> e.plan.id to i }.toMap()

    val effectivePlans = programCustomRepo.effectivePlanForDay(dayKey)
    val effectiveIds = effectivePlans.mapTo(mutableSetOf()) { it.id }
    // Logged exercises added mid-session that aren't part of the day's plan (e.g. a lift picked
    // from another day) — render them too, resolved from the library, so they don't silently
    // vanish on refresh (leaving an invisible orphan row). Resolve by the SLOT id (not exercise_id):
    // a swapped entry's exercise_id is the swapped exercise, but it's matched below via bySlotId, so
    // the extra plan must carry the slot id or the row orphans again (#11).
    // Iterate the deduped bySlotId keys, not the raw logged rows: when two rows collide on the
    // same out-of-plan effectiveSlotId (a stale pre-v22 row, a double cross-day add), the raw list
    // would emit one identical extra plan per row and render duplicate cards. bySlotId already
    // collapsed the collision to one entry.
    val extraPlans = bySlotId.keys
        .filterNot { it in effectiveIds }
        .mapNotNull { Program.exercise(it) }
    val allPlans = effectivePlans + extraPlans

    // Each card's build is independent DB reads — fan them out concurrently instead of
    // deriving the day one exercise at a time (the dominant cost of opening the screen).
    val plateLb = settingsRepo.plateWeightLb.first()
    val dbMaxLb = settingsRepo.maxDbWeightLb.first()
    val weightUnit = settingsRepo.weightUnit.first()
    val built = coroutineScope {
        allPlans.mapIndexed { index, plan ->
            async {
                buildExerciseUi(
                    plan = plan,
                    logged = bySlotId[plan.id],
                    expandedDefault = (index == 0),
                    expandedOverride = previousExpandedById[plan.id],
                    plateLb = plateLb,
                    dbMaxLb = dbMaxLb,
                    weightUnit = weightUnit,
                    bonusSets = previousBonusById[plan.id] ?: 0,
                    finishedEarly = previousFinishedEarlyById[plan.id] ?: false
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
    // The warmup is derived from these exercises and their working loads, so it is rebuilt here
    // rather than guessed at construction. No-ops once the user has stepped into it.
    rebuildWarmupProtocol()
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
    // Match by SLOT id — a swapped entry's exercise_id is the swapped exercise (#11). On an
    // effectiveSlotId collision, pick the entry with the MOST logged sets — the same policy the
    // full refreshExercises uses — so the just-logged set never hides behind an empty swap stub
    // (firstOrNull picked by order_index, which could return the empty row). setsFor only runs on
    // an actual collision.
    val matching = workoutRepo.loggedExercisesForSession(sessionId)
        .filter { it.effectiveSlotId == exerciseId }
    val logged = if (matching.size <= 1) matching.firstOrNull()
        else matching.maxByOrNull { workoutRepo.setsFor(it.id).size }
    val rebuilt = buildExerciseUi(
        plan = existing.plan,
        logged = logged,
        expandedDefault = idx == 0,
        expandedOverride = existing.isExpanded,
        plateLb = settingsRepo.plateWeightLb.first(),
        dbMaxLb = settingsRepo.maxDbWeightLb.first(),
        weightUnit = settingsRepo.weightUnit.first(),
        bonusSets = existing.bonusSets,
        finishedEarly = existing.finishedEarly
    )
    // Splice into the list as it is AT WRITE TIME, not into `current` — which was snapshotted
    // before roughly seven suspending DB reads above. Rebuilding from the stale snapshot published
    // it back wholesale, so a set logged on a DIFFERENT exercise while those reads were in flight
    // (superset alternation is the everyday case) was erased from state: still in the database,
    // gone from the screen, and gone from the totals the finish path used to read off this list.
    //
    // _state.update retries its block on contention, so re-finding the index inside it is what
    // makes the splice safe rather than merely narrower.
    _state.update { s ->
        val list = s.exercises.toMutableList()
        val at = list.indexOfFirst { it.plan.id == exerciseId }
        if (at < 0) s
        else {
            list[at] = rebuilt
            s.copy(isLoading = false, exercises = annotateNextExerciseDeltas(list))
        }
    }
}

/** Resolve the exercise that owns [setId] and rebuild just it (per-set edits). */
internal suspend fun DayViewModel.refreshExerciseForSet(setId: Long) {
    val exId = findExerciseIdForSet(setId)
    if (exId != null) refreshExercise(exId) else refreshExercises()
}

internal fun DayViewModel.findExerciseIdForSet(setId: Long): String? =
    _state.value.exercises.firstOrNull { ex -> ex.loggedSets.any { it.id == setId } }?.plan?.id

/**
 * The slot's logged_exercise row id, creating it lazily on first use.
 *
 * Serialised, and with a DATABASE check before inserting. Both matter, for different reasons:
 *
 * `loggedExerciseId` comes from UI state, which only refreshes after a write completes. Two rapid
 * LOG SET taps (or a superset alternating between two cards) both read null and both INSERT,
 * leaving two rows for one slot. refreshExercises then keeps whichever has more sets, and the sets
 * on the other row become invisible — still in the database, absent from the screen and from the
 * session's totals. logged_exercise has no unique index on (session, slot) to catch it.
 *
 * The mutex alone would not fix that: the second caller would wait, then still read a stale null
 * from UI state. So inside the lock we ask Room, not the UI. And the second caller must WAIT rather
 * than bail out — returning null there would drop the user's set, which is worse than the race.
 *
 * DaySwapHandlers guards its own paths with `swapsInFlight` for the same underlying bug; this is
 * the set-logging half, which never had a guard.
 */
internal suspend fun DayViewModel.ensureLoggedExercise(exerciseId: String): Long? =
    loggedExerciseMutex.withLock {
        val sessionId = _state.value.sessionId ?: return@withLock null
        val currentUi = _state.value.exercises.firstOrNull { it.plan.id == exerciseId }
            ?: return@withLock null
        // Log under the REAL exercise (the swapped one when a persistent swap is active), stashing the
        // slot id so the slot stays mapped (#11). `exerciseId` here is the slot (plan.id).
        val effective = currentUi.effectiveExerciseId.ifBlank { exerciseId }
        currentUi.loggedExerciseId
            // Read-and-create in ONE transaction. The mutex above serialises this ViewModel's own
            // callers; it says nothing about the WRIST, whose commands run through SetLogUseCase in
            // this same process and reach the same slot by a path that never took this lock.
            ?: workoutRepo.ensureLoggedExerciseForSlot(
                sessionId = sessionId,
                slotId = exerciseId,
                exerciseId = effective,
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
        ?: return
    // Task 3: if the user curated this day's order, ASK before changing it (surface the prompt).
    // If it's the default generated order, just apply the better order silently — no prompt.
    if (programCustomRepo.isOrderCustomized(dayKey)) {
        _state.update { it.copy(orderingSuggestion = suggestion) }
    } else {
        applyOrderedExercises(suggestion)
    }
}

internal fun DayViewModel.startSessionService(dayName: String) {
    bridge.startSession(SessionNotifState(dayName, clock.nowMs()))
    appContext.startForegroundService(Intent(appContext, WorkoutSessionService::class.java))
}

internal fun DayViewModel.stopSessionService() {
    bridge.endSession()
    // The shared timer is app-scoped now (W1) — a session that ends takes its rest timer with it,
    // on the phone AND the wrist (the publisher deletes /timer/state when this goes null).
    restTimer.stop()
    appContext.stopService(Intent(appContext, WorkoutSessionService::class.java))
}
