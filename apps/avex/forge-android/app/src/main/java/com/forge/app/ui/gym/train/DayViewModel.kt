package com.forge.app.ui.gym.train

import android.content.Context
import android.content.Intent
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.core.time.Clock
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import com.forge.app.data.repo.CustomizationRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.data.repo.TrophyRepository
import com.forge.app.data.repo.WorkoutRepository
import com.forge.app.domain.mood.Mood
import com.forge.app.domain.parser.WeightParser
import com.forge.app.domain.pr.PrDetector
import com.forge.app.domain.timer.RestTimerController
import com.forge.app.domain.volume.VolumeCalculator
import com.forge.app.program.ExercisePlan
import com.forge.app.program.MuscleGroup
import com.forge.app.program.Program
import com.forge.app.program.Swap
import com.forge.app.service.SessionNotifState
import com.forge.app.service.WorkoutSessionBridge
import com.forge.app.service.WorkoutSessionService
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.DayUiState
import com.forge.app.ui.gym.train.state.ExerciseHighlight
import com.forge.app.ui.gym.train.state.ExerciseUiState
import com.forge.app.ui.gym.train.state.SessionSummary
import com.forge.app.ui.gym.train.state.UnlockedTrophyHighlight
import com.forge.app.ui.gym.train.state.VsLastStatus
import com.forge.app.ui.nav.Routes
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
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
 *  - session lifecycle (start / resume / finish / discard / save-and-exit)
 *  - per-exercise edits (rating, note, skip, undo)
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
    private val goalRepo: GoalRepository,
    private val settingsRepo: com.forge.app.data.prefs.SettingsRepository,
    private val warmupRepo: com.forge.app.data.repo.WarmupRepository,
    private val clock: Clock,
    @ApplicationContext private val appContext: Context,
    private val bridge: WorkoutSessionBridge,
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

    /** Cancels the 5-second undo window after a set is logged. */
    private var undoClearJob: Job? = null

    init {
        var prevTimerFinished = false
        viewModelScope.launch {
            restTimer.state.collect { timer ->
                val justFinished = !prevTimerFinished && (timer?.isFinished == true)
                prevTimerFinished = timer?.isFinished == true
                if (justFinished) bridge.notifyTimerDone()
                _state.update { it.copy(restTimer = timer) }
            }
        }

        viewModelScope.launch {
            val sessionId = workoutRepo.startOrResumeSession(dayKey)
            val nameOverride = customizationRepo.getDayName(dayKey)
            val resolvedName = nameOverride?.customName ?: dayPlan.defaultName
            _state.update {
                it.copy(
                    sessionId = sessionId,
                    sessionStartedAt = clock.nowMs(),
                    displayName = resolvedName,
                    isWarmupComplete = it.isWarmupComplete || skipWarmup
                )
            }
            refreshExercises()
            startSessionService(resolvedName)
            val isNewSession = workoutRepo.isNewSession(sessionId)
            if (isNewSession) _state.update { it.copy(showPreSessionPicker = true) }
        }

        // Load custom warmup items — DB-backed (#144) takes priority over DataStore (#120)
        viewModelScope.launch {
            val dbWarmup = warmupRepo.customWarmupForDay(dayKey)
            if (dbWarmup != null) {
                _state.update { it.copy(customWarmupItems = dbWarmup) }
            } else {
                settingsRepo.getCustomWarmup(dayKey).collect { custom ->
                    _state.update { it.copy(customWarmupItems = custom) }
                }
            }
        }
    }

    fun onEvent(event: DayUiEvent) {
        when (event) {
            is DayUiEvent.ToggleExpanded -> toggleExpanded(event.exerciseId)
            is DayUiEvent.LogSet -> logSet(event.exerciseId, event.weightText, event.reps)
            is DayUiEvent.LogSameAsLast -> logSameAsLast(event.exerciseId, event.setId)
            is DayUiEvent.DeleteSet -> deleteSet(event.setId)
            is DayUiEvent.EditSet -> editSet(event.setId, event.weightText, event.reps)
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
            is DayUiEvent.RestTimerAddSeconds -> restTimer.addSeconds(event.seconds)

            DayUiEvent.FinishWorkout -> finishWorkout()
            is DayUiEvent.DismissSummary -> dismissSummary(event.mood, event.tags)
            DayUiEvent.RequestBack -> requestBack()
            DayUiEvent.ConfirmDiscard -> discardAndExit()
            DayUiEvent.DismissDiscardConfirm -> _state.update { it.copy(showDiscardConfirm = false) }
            DayUiEvent.SaveAndExit -> saveAndExit()
            DayUiEvent.UndoLastSet -> undoLastSet()
            is DayUiEvent.OpenGoalSetter -> _state.update { it.copy(goalSetterForExerciseId = event.exerciseId) }
            is DayUiEvent.SetGoal -> {
                viewModelScope.launch {
                    goalRepo.setGoal(event.exerciseId, event.targetWeightLb)
                    _state.update { it.copy(goalSetterForExerciseId = null) }
                    refreshExercises()
                }
            }
            is DayUiEvent.ClearGoal -> {
                viewModelScope.launch {
                    goalRepo.clearGoal(event.exerciseId)
                    _state.update { it.copy(goalSetterForExerciseId = null) }
                    refreshExercises()
                }
            }
            DayUiEvent.DismissGoalSetter -> _state.update { it.copy(goalSetterForExerciseId = null) }
            is DayUiEvent.SetExerciseUnit -> {
                viewModelScope.launch {
                    val existing = customizationRepo.getSwap(event.exerciseId)
                    if (event.unit == null) {
                        if (existing?.swappedName?.isBlank() == true) customizationRepo.clearSwap(event.exerciseId)
                        // else keep the swap but clear unit — upsert with empty unit
                        existing?.let { customizationRepo.setSwap(it.exerciseId, it.swappedName, "") }
                    } else {
                        customizationRepo.setSwap(event.exerciseId, existing?.swappedName ?: "", event.unit)
                    }
                    refreshExercises()
                }
            }
            is DayUiEvent.SetRestTimerOverride -> {
                viewModelScope.launch {
                    customizationRepo.setRestTimerOverride(event.exerciseId, event.seconds)
                    refreshExercises()
                }
            }
            DayUiEvent.ConfirmWeightJump -> {
                val warning = _state.value.pendingWeightJumpWarning ?: return
                _state.update { it.copy(pendingWeightJumpWarning = null) }
                logSet(warning.exerciseId, warning.weightText, warning.reps)
            }
            DayUiEvent.DismissWeightJump -> _state.update { it.copy(pendingWeightJumpWarning = null) }
            is DayUiEvent.SetSessionType -> {
                _state.update { it.copy(sessionType = event.type) }
                _state.value.sessionId?.let { id -> viewModelScope.launch { workoutRepo.setSessionType(id, event.type) } }
            }
            is DayUiEvent.SetUntracked -> {
                _state.update { it.copy(isUntracked = event.v) }
                _state.value.sessionId?.let { id -> viewModelScope.launch { workoutRepo.setUntracked(id, event.v) } }
            }
            is DayUiEvent.SetIntensity -> {
                _state.update { it.copy(sessionIntensity = event.intensity) }
                _state.value.sessionId?.let { id -> viewModelScope.launch { workoutRepo.setIntensity(id, event.intensity) } }
            }
            DayUiEvent.ConfirmPreSessionPicker -> _state.update { it.copy(showPreSessionPicker = false) }
            is DayUiEvent.UpdateJournal -> {
                _state.value.sessionId?.let { id -> viewModelScope.launch { workoutRepo.setJournal(id, event.text) } }
            }
            is DayUiEvent.SetPinnedNote -> {
                viewModelScope.launch {
                    customizationRepo.setPinnedNote(event.exerciseId, event.note)
                    refreshExercises()
                }
            }
            is DayUiEvent.MoveExercise -> {
                val exercises = _state.value.exercises.toMutableList()
                val idx = exercises.indexOfFirst { it.plan.id == event.exerciseId }
                val newIdx = (idx + event.direction).coerceIn(0, exercises.lastIndex)
                if (idx != newIdx) {
                    val moved = exercises.removeAt(idx)
                    exercises.add(newIdx, moved)
                    _state.update { it.copy(exercises = exercises) }
                }
            }
            is DayUiEvent.LongPressExercise -> _state.update { it.copy(quickActionsForExerciseId = event.exerciseId) }
            DayUiEvent.DismissQuickActions -> _state.update { it.copy(quickActionsForExerciseId = null) }
            DayUiEvent.OpenAddExercisePicker -> _state.update { it.copy(showAddExercisePicker = true) }
            DayUiEvent.CloseAddExercisePicker -> _state.update { it.copy(showAddExercisePicker = false) }
            is DayUiEvent.SetSupersetGroup -> viewModelScope.launch {
                val loggedId = _state.value.exercises.firstOrNull { it.plan.id == event.exerciseId }?.loggedExerciseId
                    ?: return@launch
                workoutRepo.setSupersetGroup(loggedId, event.group)
                refreshExercises()
            }
            is DayUiEvent.ShowWarmupSuggester -> _state.update { it.copy(warmupSuggesterForExerciseId = event.exerciseId) }
            is DayUiEvent.ShowPlateCalculator -> _state.update { it.copy(plateCalculatorForExerciseId = event.exerciseId) }
            DayUiEvent.DismissTrainingHelper -> _state.update { it.copy(warmupSuggesterForExerciseId = null, plateCalculatorForExerciseId = null) }
            is DayUiEvent.LogBreak -> {
                val sessionId = _state.value.sessionId ?: return
                viewModelScope.launch { workoutRepo.logBreak(sessionId, event.type) }
            }
            is DayUiEvent.ToggleAmrap -> viewModelScope.launch {
                val set = findSet(event.setId) ?: return@launch
                workoutRepo.setAmrap(event.setId, !set.isAmrap)
                refreshExercises()
            }
            is DayUiEvent.ToggleAssisted -> viewModelScope.launch {
                val set = findSet(event.setId) ?: return@launch
                workoutRepo.setAssisted(event.setId, !set.isAssisted)
                refreshExercises()
            }
            is DayUiEvent.ToggleFailure -> viewModelScope.launch {
                val set = findSet(event.setId) ?: return@launch
                workoutRepo.setToFailure(event.setId, !set.toFailure)
                refreshExercises()
            }
            is DayUiEvent.SetSetType -> viewModelScope.launch {
                workoutRepo.setSetType(event.setId, event.type)
                refreshExercises()
            }
            is DayUiEvent.SetDropAnnotation -> viewModelScope.launch {
                workoutRepo.setDropAnnotation(event.setId, event.annotation)
                refreshExercises()
            }
            is DayUiEvent.AddUnplannedExercise -> {
                viewModelScope.launch {
                    val sessionId = _state.value.sessionId ?: return@launch
                    val existing = _state.value.exercises
                    // Don't add the same exercise twice in one session
                    if (existing.any { it.plan.id == event.exerciseId }) {
                        _state.update { it.copy(showAddExercisePicker = false) }
                        return@launch
                    }
                    val plan = Program.exercise(event.exerciseId) ?: return@launch
                    workoutRepo.addExerciseToSession(
                        sessionId = sessionId,
                        exerciseId = event.exerciseId,
                        orderIndex = existing.size
                    )
                    _state.update { it.copy(showAddExercisePicker = false) }
                    refreshExercises()
                }
            }
            is DayUiEvent.WarmupReaction -> {
                val current = _state.value.warmupReactions.toMutableMap()
                if (current[event.index] == event.thumbsUp) current.remove(event.index)
                else current[event.index] = event.thumbsUp
                _state.update { it.copy(warmupReactions = current) }
            }
            is DayUiEvent.ToggleSetDifficultyTag -> {
                viewModelScope.launch {
                    val nextTag = when (event.currentTag) {
                        null -> "easy"
                        "easy" -> "hard"
                        else -> null
                    }
                    workoutRepo.setDifficultyTag(event.setId, nextTag)
                    refreshExercises()
                }
            }
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

        // Fetch all historical sets once; split into prior (other sessions) and all-time
        val allHistory = workoutRepo.historyForExercise(plan.id)
        val prior = allHistory.filter { it.loggedExerciseId != logged?.id }

        val (prSetIds, wasPr) = computePrFlags(prior, sets)

        // Persist wasPr flag if it changed — trophy count reads this column later.
        if (logged != null && logged.wasPr != wasPr) {
            workoutRepo.updateExercise(logged.copy(wasPr = wasPr))
        }

        val (suggestedWeight, suggestionReason) = computeWeightSuggestion(plan, prevLE, prevSets)

        // All-time PB: best weight ever × max reps at that weight (#101)
        val pbSet = allHistory.filter { it.weightLb != null }.maxByOrNull { it.weightLb!! }
        val allTimePbLb = pbSet?.weightLb
        val allTimePbText = pbSet?.let { best ->
            val maxRepsAtWeight = allHistory
                .filter { it.weightLb == best.weightLb }
                .maxOf { it.reps }
            "${best.weightText} × $maxRepsAtWeight"
        }
        val goalWeightLb = goalRepo.get(plan.id)?.targetWeightLb

        // Vs last session volume comparison (#104)
        val currentVolume = sets.sumOf { (it.weightLb ?: 0.0) * it.reps }
        val prevVolume = prevSets.sumOf { (it.weightLb ?: 0.0) * it.reps }
        val vsLastStatus = when {
            sets.isEmpty() -> null
            prevSets.isEmpty() -> null
            currentVolume > prevVolume * 1.05 -> VsLastStatus.BEATING
            currentVolume >= prevVolume * 0.95 -> VsLastStatus.MATCHING
            else -> VsLastStatus.UNDER
        }

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
            suggestionReason = suggestionReason,
            priorSets = prior,
            allTimePbText = allTimePbText,
            allTimePbLb = allTimePbLb,
            vsLastStatus = vsLastStatus,
            goalWeightLb = goalWeightLb,
            restTimerOverrideSeconds = persistent?.restTimerOverrideSeconds,
            pinnedNote = persistent?.pinnedNote ?: "",
            supersetGroup = logged?.supersetGroup
        )
    }

    private fun computeTimerDuration(plan: ExercisePlan, effortRating: EffortRating?, overrideSeconds: Int? = null): Int {
        if (overrideSeconds != null) return overrideSeconds
        val base = when (plan.muscle) {
            MuscleGroup.CHEST, MuscleGroup.BACK,
            MuscleGroup.QUADS, MuscleGroup.HAMSTRINGS,
            MuscleGroup.GLUTES -> 180
            else -> 90
        }
        return base + if (effortRating == EffortRating.BRUTAL) 30 else 0
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

        // Intensity auto-adjust (#145): scale suggestion based on declared session intensity
        val intensityMultiplier = when (_state.value.sessionIntensity) {
            "light" -> 0.9   // suggest 10% lighter on light days
            "hard"  -> 1.05  // suggest 5% heavier on hard days
            else    -> 1.0
        }
        val suggested = ((prevMaxWeight + adjustmentLb) * intensityMultiplier)
            .let { w -> (w / 2.5).toInt() * 2.5 } // round to nearest 2.5 lb
        if (suggested <= 0.0) return null to null
        val adjustedReason = if (intensityMultiplier != 1.0)
            "$reason · intensity adjusted" else reason
        val suggestedStr = if (suggested % 1.0 == 0.0) "${suggested.toInt()}" else "$suggested"
        return suggestedStr to adjustedReason
    }

    private fun parseMaxReps(repsText: String): Int? =
        repsText.split(Regex("[^0-9]+")).mapNotNull { it.toIntOrNull() }.maxOrNull()

    /** Returns (prSetIds, wasPr). PRs are evaluated against [prior] sets from other sessions. */
    private fun computePrFlags(
        prior: List<LoggedSet>,
        currentSets: List<LoggedSet>
    ): Pair<Set<Long>, Boolean> {
        if (currentSets.isEmpty()) return emptySet<Long>() to false
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

            // Weight-jump warning (#117): flag if new weight > 120% of previous best
            val newWeightLb = WeightParser.parse(weightText, plan.unit)
            val lastWeightLb = currentUi.priorSets.mapNotNull { it.weightLb }.maxOrNull()
            if (newWeightLb != null && lastWeightLb != null && lastWeightLb > 0 &&
                newWeightLb > lastWeightLb * 1.20
            ) {
                _state.update {
                    it.copy(pendingWeightJumpWarning = com.forge.app.ui.gym.train.state.WeightJumpWarning(
                        exerciseId = exerciseId,
                        weightText = weightText,
                        reps = reps,
                        lastWeightLb = lastWeightLb,
                        newWeightLb = newWeightLb
                    ))
                }
                return@launch
            }

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

            restTimer.start(computeTimerDuration(plan, currentUi.difficulty, currentUi.restTimerOverrideSeconds))
            refreshExercises()

            // Auto-collapse when exactly hitting the planned set count (#98)
            val updatedEx = _state.value.exercises.firstOrNull { it.plan.id == exerciseId }
            if (updatedEx != null && updatedEx.loggedSets.size == plan.sets) {
                _state.update { s ->
                    s.copy(exercises = s.exercises.map {
                        if (it.plan.id == exerciseId) it.copy(isExpanded = false) else it
                    })
                }
            }

            // Record undo window (#46): store the last set ID, clear after 5s
            val newSetId = _state.value.exercises
                .firstOrNull { it.plan.id == exerciseId }
                ?.loggedSets?.lastOrNull()?.id
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

    private fun editSet(setId: Long, weightText: String, reps: Int) {
        if (reps <= 0) return
        viewModelScope.launch {
            val exerciseUi = _state.value.exercises.firstOrNull { ex ->
                ex.loggedSets.any { it.id == setId }
            } ?: return@launch
            val plan = dayPlan.exercises.firstOrNull { it.id == exerciseUi.plan.id } ?: return@launch
            val set = exerciseUi.loggedSets.firstOrNull { it.id == setId } ?: return@launch
            val weightLb = WeightParser.parse(weightText, plan.unit)
            workoutRepo.updateSet(set.copy(weightText = weightText, weightLb = weightLb, reps = reps))
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
            val newSkipped = !currentUi.skipped
            workoutRepo.setSkipped(leId, newSkipped)
            refreshExercises()

            // Auto-expand the next incomplete exercise after skipping (#49)
            if (newSkipped) {
                val nextEx = _state.value.exercises
                    .dropWhile { it.plan.id != exerciseId }
                    .drop(1)
                    .firstOrNull { !it.skipped && it.loggedSets.size < it.plan.sets }
                if (nextEx != null) {
                    _state.update { s ->
                        s.copy(exercises = s.exercises.map {
                            if (it.plan.id == nextEx.plan.id) it.copy(isExpanded = true) else it
                        })
                    }
                }
            }
        }
    }

    // ─── Events: swap ─────────────────────────────────────────────────────────

    private fun applySessionSwap(exerciseId: String, swap: Swap) {
        viewModelScope.launch {
            val leId = ensureLoggedExercise(exerciseId) ?: return@launch
            val le = _state.value.exercises.firstOrNull { it.plan.id == exerciseId }?.loggedExerciseId
            if (le != null) {
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

            workoutRepo.finishSession(sessionId, totalVolumeLb, prCount, allSets.size)
            restTimer.stop()
            stopSessionService()

            val newlyUnlocked = trophyRepo.evaluateAndUnlockNew().map { t ->
                UnlockedTrophyHighlight(
                    id = t.id,
                    name = t.name,
                    description = t.description,
                    icon = t.icon
                )
            }

            // Session comparison (#52) and best session callout (#53)
            val prevSession = workoutRepo.previousSessionForDay(dayKey, sessionId)
            val vsLastVolumeDelta = prevSession?.totalVolumeLb?.let { totalVolumeLb - it }
            val vsLastSetsDelta = prevSession?.setCount?.let { allSets.size - it }
                ?.takeIf { prevSession.setCount > 0 }
            val bestPrevVolume = workoutRepo.bestPreviousVolumeForDay(dayKey, sessionId) ?: 0.0
            val isBestSession = prevSession != null && totalVolumeLb > bestPrevVolume

            val durationMin = ((clock.nowMs() - startedAt) / 60_000).toInt().coerceAtLeast(0)

            // Pace + density (#83, #127)
            val setsPerMin = if (durationMin > 0) allSets.size.toDouble() / durationMin else 0.0
            val volumePerMin = if (durationMin > 0) totalVolumeLb / durationMin else 0.0
            val densityScore = if (durationMin > 0) totalVolumeLb / durationMin else null

            // Actual avg rest time from per-set timestamps (#82)
            val sessionSetsOrdered = workoutRepo.allSetsForSession(sessionId)
            val avgRestSeconds = computeAvgRestSeconds(sessionSetsOrdered)

            // Honesty index: planned sets vs actually logged (#133)
            val exercises = _state.value.exercises
            val plannedTotal = exercises.filter { !it.skipped }.sumOf { it.plan.sets }
            val loggedNonSkipped = exercises.filter { !it.skipped }.sumOf { it.loggedSets.size }
            val honestyPct = if (plannedTotal > 0)
                ((loggedNonSkipped.toDouble() / plannedTotal) * 100).toInt().coerceIn(0, 100)
            else null

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

    /** Save immediately and exit without showing the summary sheet (#97). */
    private fun saveAndExit() {
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
            trophyRepo.evaluateAndUnlockNew() // evaluate but don't surface results
            _state.update { it.copy(isFinished = true) }
            _navigation.send(DayNavigationEffect.PopBack)
        }
    }

    /** Undo the most recently logged set within the 5s window (#46). */
    private fun undoLastSet() {
        val setId = _state.value.undoableSetId ?: return
        undoClearJob?.cancel()
        _state.update { it.copy(undoableSetId = null) }
        deleteSet(setId)
    }

    private fun dismissSummary(mood: Mood?, tags: List<String>) {
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

    private fun requestBack() {
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

    private fun discardAndExit() {
        viewModelScope.launch {
            val sessionId = _state.value.sessionId ?: return@launch
            workoutRepo.discardSession(sessionId)
            restTimer.stop()
            stopSessionService()
            _state.update { it.copy(showDiscardConfirm = false) }
            _navigation.send(DayNavigationEffect.PopBack)
        }
    }

    // ─── Helpers ──────────────────────────────────────────────────────────────

    private fun startSessionService(dayName: String) {
        bridge.startSession(SessionNotifState(dayName, clock.nowMs()))
        appContext.startForegroundService(Intent(appContext, WorkoutSessionService::class.java))
    }

    private fun stopSessionService() {
        bridge.endSession()
        appContext.stopService(Intent(appContext, WorkoutSessionService::class.java))
    }

    private fun findSet(setId: Long): com.forge.app.data.db.entities.LoggedSet? =
        _state.value.exercises.flatMap { it.loggedSets }.firstOrNull { it.id == setId }

    /**
     * Given all sets in a session ordered by completedAt, groups them by loggedExerciseId and
     * computes the average gap between consecutive sets within each exercise (#82).
     */
    private fun computeAvgRestSeconds(sets: List<com.forge.app.data.db.entities.LoggedSet>): Int? {
        val gaps = sets
            .groupBy { it.loggedExerciseId }
            .values
            .flatMap { exerciseSets ->
                val sorted = exerciseSets.sortedBy { it.completedAt }
                sorted.zipWithNext { a, b -> (b.completedAt - a.completedAt) / 1000L }
                    .filter { it in 5..600 } // ignore gaps outside 5s–10min (pauses, interruptions)
            }
        return if (gaps.isEmpty()) null else (gaps.average()).toInt()
    }

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
