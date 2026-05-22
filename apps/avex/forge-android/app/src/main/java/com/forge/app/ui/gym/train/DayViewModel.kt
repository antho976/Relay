package com.forge.app.ui.gym.train

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.core.time.Clock
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.types.EffortRating
import com.forge.app.data.repo.CustomizationRepository
import com.forge.app.data.repo.TrophyRepository
import com.forge.app.data.repo.WorkoutRepository
import com.forge.app.domain.mood.Mood
import com.forge.app.domain.parser.WeightParser
import com.forge.app.domain.pr.PrDetector
import com.forge.app.domain.timer.RestTimerController
import com.forge.app.domain.volume.VolumeCalculator
import com.forge.app.program.ExercisePlan
import com.forge.app.program.Program
import com.forge.app.program.Swap
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.DayUiState
import com.forge.app.ui.gym.train.state.ExerciseHighlight
import com.forge.app.ui.gym.train.state.ExerciseUiState
import com.forge.app.ui.gym.train.state.SessionSummary
import com.forge.app.ui.gym.train.state.UnlockedTrophyHighlight
import com.forge.app.ui.nav.Routes
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Phase 3c — active workout state holder. Coordinates:
 *  - session lifecycle (start / resume / finish / discard)
 *  - per-exercise edits (rating, note, skip)
 *  - swap state (session-only via LoggedExercise.swappedName, persistent via ExerciseCustomization)
 *  - PR detection on every refresh
 *  - end-of-workout volume calc + summary sheet
 *  - rest timer (delegated to RestTimerController)
 *  - warmup gate
 *
 * Larger than the 150-line VM guideline by design: it's a linear dispatch of ~20
 * small event handlers, no nested logic. Splitting handlers across files would add
 * indirection without simplifying anything.
 */
@HiltViewModel
class DayViewModel @Inject constructor(
    private val workoutRepo: WorkoutRepository,
    private val customizationRepo: CustomizationRepository,
    private val trophyRepo: TrophyRepository,
    private val clock: Clock,
    savedStateHandle: SavedStateHandle
) : ViewModel() {

    private val dayKey: String = savedStateHandle.get<String>(Routes.ARG_DAY_KEY)
        ?: error("dayKey missing from SavedStateHandle")
    private val dayPlan = Program.day(dayKey)
    private val skipWarmup: Boolean = savedStateHandle.get<Boolean>(Routes.ARG_SKIP_WARMUP) ?: false

    private val restTimer = RestTimerController(viewModelScope)

    private val _state = MutableStateFlow(
        DayUiState(dayPlan = dayPlan, displayName = dayPlan.defaultName)
    )
    val state: StateFlow<DayUiState> = _state.asStateFlow()

    private val _navigation = Channel<DayNavigationEffect>(Channel.BUFFERED)
    val navigationEffects = _navigation.receiveAsFlow()

    init {
        viewModelScope.launch {
            restTimer.state.collect { timer ->
                _state.update { it.copy(restTimer = timer) }
            }
        }

        viewModelScope.launch {
            val sessionId = workoutRepo.startOrResumeSession(dayKey)
            val nameOverride = customizationRepo.getDayName(dayKey)
            _state.update {
                it.copy(
                    sessionId = sessionId,
                    sessionStartedAt = clock.nowMs(), // refined below if resuming
                    displayName = nameOverride?.customName ?: dayPlan.defaultName,
                    isWarmupComplete = it.isWarmupComplete || skipWarmup
                )
            }
            refreshExercises()
        }
    }

    fun onEvent(event: DayUiEvent) {
        when (event) {
            is DayUiEvent.ToggleExpanded -> toggleExpanded(event.exerciseId)
            is DayUiEvent.LogSet -> logSet(event.exerciseId, event.weightText, event.reps)
            is DayUiEvent.LogSameAsLast -> logSameAsLast(event.exerciseId, event.setId)
            is DayUiEvent.DeleteSet -> deleteSet(event.setId)
            is DayUiEvent.RateExercise -> rateExercise(event.exerciseId, event.rating)
            is DayUiEvent.UpdateNote -> updateNote(event.exerciseId, event.note)
            is DayUiEvent.ToggleSkipped -> toggleSkipped(event.exerciseId)

            is DayUiEvent.OpenSwapPicker -> _state.update { it.copy(swapPickerForExerciseId = event.exerciseId) }
            DayUiEvent.CloseSwapPicker -> _state.update { it.copy(swapPickerForExerciseId = null) }
            is DayUiEvent.PickSwapForSession -> applySessionSwap(event.exerciseId, event.swap)
            is DayUiEvent.PickSwapPersistent -> applyPersistentSwap(event.exerciseId, event.swap)
            is DayUiEvent.ClearPersistentSwap -> clearPersistentSwap(event.exerciseId)

            is DayUiEvent.ToggleWarmupItem -> toggleWarmupItem(event.index)
            DayUiEvent.SkipWarmup -> _state.update { it.copy(isWarmupComplete = true) }

            DayUiEvent.RestTimerOpen -> _state.update { it.copy(showTimerControls = true) }
            DayUiEvent.RestTimerClose -> _state.update { it.copy(showTimerControls = false) }
            DayUiEvent.RestTimerPause -> restTimer.pause()
            DayUiEvent.RestTimerResume -> restTimer.resume()
            DayUiEvent.RestTimerReset -> restTimer.reset()
            DayUiEvent.RestTimerSkip -> {
                restTimer.stop()
                _state.update { it.copy(showTimerControls = false) }
            }

            DayUiEvent.FinishWorkout -> finishWorkout()
            is DayUiEvent.DismissSummary -> dismissSummary(event.mood)
            DayUiEvent.RequestBack -> requestBack()
            DayUiEvent.ConfirmDiscard -> discardAndExit()
            DayUiEvent.DismissDiscardConfirm -> _state.update { it.copy(showDiscardConfirm = false) }
        }
    }

    // ─── Loading ───────────────────────────────────────────────────────────────

    private suspend fun refreshExercises() {
        val sessionId = _state.value.sessionId ?: return
        val loggedExercises = workoutRepo.observeExercisesForSession(sessionId).firstOrNull().orEmpty()
        val byExerciseId = loggedExercises.associateBy { it.exerciseId }
        val previousExpandedById = _state.value.exercises.associate { it.plan.id to it.isExpanded }

        val exercises = dayPlan.exercises.mapIndexed { index, plan ->
            buildExerciseUi(
                plan = plan,
                logged = byExerciseId[plan.id],
                expandedDefault = (index == 0),
                expandedOverride = previousExpandedById[plan.id]
            )
        }

        _state.update { it.copy(isLoading = false, exercises = exercises) }
    }

    private suspend fun buildExerciseUi(
        plan: ExercisePlan,
        logged: LoggedExercise?,
        expandedDefault: Boolean,
        expandedOverride: Boolean?
    ): ExerciseUiState {
        val sessionId = _state.value.sessionId ?: error("sessionId required")
        val sets = logged?.let { workoutRepo.setsFor(it.id) }.orEmpty()
        val prevLE = workoutRepo.lastLoggedExerciseBefore(plan.id, sessionId)
        val prevSets = prevLE?.let { workoutRepo.setsFor(it.id) }.orEmpty()
        val prevFirstSet = prevSets.firstOrNull()
        val preview = prevFirstSet?.let { "Last: ${it.weightText} × ${it.reps}" }
        val persistent = customizationRepo.getSwap(plan.id)

        val (prSetIds, wasPr) = computePrFlags(plan.id, logged?.id, sets)

        // Persist wasPr flag if it changed — trophy count reads this column later.
        if (logged != null && logged.wasPr != wasPr) {
            workoutRepo.updateExercise(logged.copy(wasPr = wasPr))
        }

        val (suggestedWeight, suggestionReason) = computeWeightSuggestion(plan, prevLE, prevSets)

        return ExerciseUiState(
            plan = plan,
            loggedExerciseId = logged?.id,
            loggedSets = sets,
            lastSessionPreviewText = preview,
            prefillWeight = prevFirstSet?.weightText,
            difficulty = logged?.difficulty,
            note = logged?.note,
            skipped = logged?.skipped ?: false,
            isExpanded = expandedOverride ?: expandedDefault,
            wasPr = wasPr,
            prSetIds = prSetIds,
            sessionSwapName = logged?.swappedName,
            sessionSwapUnit = logged?.swappedUnit,
            persistentSwapName = persistent?.swappedName,
            persistentSwapUnit = persistent?.swappedUnit,
            suggestedWeight = suggestedWeight,
            suggestionReason = suggestionReason
        )
    }

    /**
     * Computes an optional weight progression suggestion (#12/#13).
     * Returns (inputString, reasonLabel) where inputString is the bare number the weight field
     * should be filled with (e.g. "27.5"), and reasonLabel is a short display hint.
     * Returns null/null when no actionable change is detected (same weight is already prefilled).
     */
    private fun computeWeightSuggestion(
        plan: ExercisePlan,
        prevLE: com.forge.app.data.db.entities.LoggedExercise?,
        prevSets: List<com.forge.app.data.db.entities.LoggedSet>
    ): Pair<String?, String?> {
        if (prevLE == null || prevSets.isEmpty()) return null to null
        val prevMaxWeight = prevSets.mapNotNull { it.weightLb }.maxOrNull() ?: return null to null
        val prevMaxReps = prevSets.maxOf { it.reps }
        val planMaxReps = parseMaxReps(plan.reps)
        val hitTopOfRange = planMaxReps != null && prevMaxReps >= planMaxReps
        val difficulty = prevLE.difficulty

        val (adjustmentLb, reason) = when {
            hitTopOfRange && (difficulty == null ||
                difficulty == EffortRating.EASY ||
                difficulty == EffortRating.JUST_RIGHT) -> 2.5 to "hit top of range"
            difficulty == EffortRating.BRUTAL -> -2.5 to "last rated brutal"
            else -> return null to null
        }

        val suggested = prevMaxWeight + adjustmentLb
        if (suggested <= 0.0) return null to null
        val suggestedStr = if (suggested % 1.0 == 0.0) "${suggested.toInt()}" else "$suggested"
        return suggestedStr to reason
    }

    private fun parseMaxReps(repsText: String): Int? =
        repsText.split(Regex("[^0-9]+")).mapNotNull { it.toIntOrNull() }.maxOrNull()

    /** Returns (prSetIds, wasPr). PRs are evaluated against history from OTHER sessions only. */
    private suspend fun computePrFlags(
        exerciseId: String,
        currentLoggedExerciseId: Long?,
        currentSets: List<com.forge.app.data.db.entities.LoggedSet>
    ): Pair<Set<Long>, Boolean> {
        if (currentSets.isEmpty()) return emptySet<Long>() to false
        val allHistory = workoutRepo.historyForExercise(exerciseId)
        val prior = allHistory.filter { it.loggedExerciseId != currentLoggedExerciseId }
        val prIds = currentSets
            .filter { PrDetector.isPr(prior, it.weightLb, it.reps) }
            .map { it.id }
            .toSet()
        return prIds to prIds.isNotEmpty()
    }

    // ─── Events: exercise card ────────────────────────────────────────────────

    private fun toggleExpanded(exerciseId: String) {
        _state.update { current ->
            current.copy(
                exercises = current.exercises.map {
                    if (it.plan.id == exerciseId) it.copy(isExpanded = !it.isExpanded) else it
                }
            )
        }
    }

    private fun logSet(exerciseId: String, weightText: String, reps: Int) {
        if (reps <= 0) return
        viewModelScope.launch {
            val sessionId = _state.value.sessionId ?: return@launch
            val plan = dayPlan.exercises.firstOrNull { it.id == exerciseId } ?: return@launch
            val currentUi = _state.value.exercises.firstOrNull { it.plan.id == exerciseId }
                ?: return@launch

            val leId = currentUi.loggedExerciseId
                ?: workoutRepo.addExerciseToSession(
                    sessionId = sessionId,
                    exerciseId = exerciseId,
                    orderIndex = dayPlan.exercises.indexOfFirst { it.id == exerciseId },
                    swappedName = currentUi.sessionSwapName ?: currentUi.persistentSwapName,
                    swappedUnit = currentUi.sessionSwapUnit ?: currentUi.persistentSwapUnit
                )

            val weightLb = WeightParser.parse(weightText, plan.unit)
            workoutRepo.logSet(
                loggedExerciseId = leId,
                setIndex = currentUi.loggedSets.size,
                weightText = weightText,
                weightLb = weightLb,
                reps = reps
            )

            restTimer.start()
            refreshExercises()
        }
    }

    private fun logSameAsLast(exerciseId: String, setId: Long) {
        val set = _state.value.exercises.flatMap { it.loggedSets }.firstOrNull { it.id == setId }
            ?: return
        logSet(exerciseId, set.weightText, set.reps)
    }

    private fun deleteSet(setId: Long) {
        viewModelScope.launch {
            val set = _state.value.exercises
                .flatMap { it.loggedSets }
                .firstOrNull { it.id == setId } ?: return@launch
            workoutRepo.deleteSet(set)
            refreshExercises()
        }
    }

    private fun rateExercise(exerciseId: String, rating: EffortRating) {
        viewModelScope.launch {
            val leId = ensureLoggedExercise(exerciseId) ?: return@launch
            workoutRepo.setRating(leId, rating)
            refreshExercises()
        }
    }

    private fun updateNote(exerciseId: String, note: String) {
        viewModelScope.launch {
            val leId = ensureLoggedExercise(exerciseId) ?: return@launch
            workoutRepo.setNote(leId, note.ifBlank { null })
            refreshExercises()
        }
    }

    private fun toggleSkipped(exerciseId: String) {
        viewModelScope.launch {
            val currentUi = _state.value.exercises.firstOrNull { it.plan.id == exerciseId } ?: return@launch
            val leId = ensureLoggedExercise(exerciseId) ?: return@launch
            workoutRepo.setSkipped(leId, !currentUi.skipped)
            refreshExercises()
        }
    }

    // ─── Events: swap ─────────────────────────────────────────────────────────

    private fun applySessionSwap(exerciseId: String, swap: Swap) {
        viewModelScope.launch {
            val leId = ensureLoggedExercise(exerciseId) ?: return@launch
            val le = _state.value.exercises.firstOrNull { it.plan.id == exerciseId }?.loggedExerciseId
            if (le != null) {
                // Update existing LoggedExercise's swap fields.
                val current = _state.value.exercises.first { it.plan.id == exerciseId }
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
            }
            _state.update { it.copy(swapPickerForExerciseId = null) }
            refreshExercises()
        }
    }

    private fun applyPersistentSwap(exerciseId: String, swap: Swap) {
        viewModelScope.launch {
            customizationRepo.setSwap(exerciseId, swap.name, swap.unit.code)
            applySessionSwap(exerciseId, swap) // also apply to the live session
        }
    }

    private fun clearPersistentSwap(exerciseId: String) {
        viewModelScope.launch {
            customizationRepo.clearSwap(exerciseId)
            _state.update { it.copy(swapPickerForExerciseId = null) }
            refreshExercises()
        }
    }

    // ─── Events: warmup ───────────────────────────────────────────────────────

    private fun toggleWarmupItem(index: Int) {
        _state.update { current ->
            val updated = current.warmupChecks.toMutableList().also {
                if (index in it.indices) it[index] = !it[index]
            }
            current.copy(
                warmupChecks = updated,
                isWarmupComplete = updated.all { it }
            )
        }
    }

    // ─── Events: session lifecycle ────────────────────────────────────────────

    private fun finishWorkout() {
        viewModelScope.launch {
            val sessionId = _state.value.sessionId ?: return@launch
            val startedAt = _state.value.sessionStartedAt ?: clock.nowMs()

            val allSets = _state.value.exercises.flatMap { it.loggedSets }
            val totalVolumeLb = VolumeCalculator.sessionVolumeLb(allSets)
            val prCount = _state.value.exercises.count { it.wasPr }

            workoutRepo.finishSession(sessionId, totalVolumeLb, prCount)
            restTimer.stop()

            val newlyUnlocked = trophyRepo.evaluateAndUnlockNew().map { t ->
                UnlockedTrophyHighlight(
                    id = t.id,
                    name = t.name,
                    description = t.description,
                    icon = t.icon
                )
            }

            val durationMin = ((clock.nowMs() - startedAt) / 60_000).toInt().coerceAtLeast(0)
            val summary = SessionSummary(
                displayName = _state.value.displayName,
                dayWord = dayPlan.word,
                durationMinutes = durationMin,
                totalVolumeLb = totalVolumeLb,
                prCount = prCount,
                setCount = allSets.size,
                exercisesLogged = _state.value.exercises.count { it.loggedSets.isNotEmpty() && !it.skipped },
                exercisesSkipped = _state.value.exercises.count { it.skipped },
                highlights = _state.value.exercises
                    .filter { it.loggedSets.isNotEmpty() || it.skipped }
                    .map { ex ->
                        ExerciseHighlight(
                            exerciseName = ex.effectiveName,
                            setsLogged = ex.loggedSets.size,
                            volumeLb = VolumeCalculator.sessionVolumeLb(ex.loggedSets),
                            isPr = ex.wasPr
                        )
                    },
                unlockedTrophies = newlyUnlocked
            )
            _state.update { it.copy(isFinished = true, summary = summary) }
        }
    }

    private fun dismissSummary(mood: Mood?) {
        viewModelScope.launch {
            val sessionId = _state.value.sessionId
            if (mood != null && sessionId != null) {
                workoutRepo.recordMood(sessionId, dayKey, mood.code)
            }
            _state.update { it.copy(summary = null) }
            _navigation.send(DayNavigationEffect.PopBack)
        }
    }

    private fun requestBack() {
        if (_state.value.hasUnsavedWork) {
            _state.update { it.copy(showDiscardConfirm = true) }
        } else {
            viewModelScope.launch {
                val sessionId = _state.value.sessionId
                if (sessionId != null) workoutRepo.discardSession(sessionId)
                restTimer.stop()
                _navigation.send(DayNavigationEffect.PopBack)
            }
        }
    }

    private fun discardAndExit() {
        viewModelScope.launch {
            val sessionId = _state.value.sessionId ?: return@launch
            workoutRepo.discardSession(sessionId)
            restTimer.stop()
            _state.update { it.copy(showDiscardConfirm = false) }
            _navigation.send(DayNavigationEffect.PopBack)
        }
    }

    // ─── Helpers ──────────────────────────────────────────────────────────────

    /** Lazily creates a LoggedExercise if none exists yet, so subsequent writes have somewhere to land. */
    private suspend fun ensureLoggedExercise(exerciseId: String): Long? {
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
}

sealed interface DayNavigationEffect {
    data object PopBack : DayNavigationEffect
}
