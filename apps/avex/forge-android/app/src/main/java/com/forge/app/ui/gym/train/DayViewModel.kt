package com.forge.app.ui.gym.train

import android.content.Context
import android.content.Intent
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.core.time.Clock
import com.forge.app.data.repo.CustomizationRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.data.repo.TrophyRepository
import com.forge.app.data.repo.WorkoutRepository
import com.forge.app.domain.timer.RestTimerController
import com.forge.app.program.Program
import com.forge.app.service.SessionNotifState
import com.forge.app.service.WorkoutSessionBridge
import com.forge.app.service.WorkoutSessionService
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.DayUiState
import com.forge.app.ui.nav.Routes
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class DayViewModel @Inject constructor(
    internal val workoutRepo: WorkoutRepository,
    internal val customizationRepo: CustomizationRepository,
    internal val programCustomRepo: com.forge.app.data.repo.ProgramCustomizationRepository,
    internal val trophyRepo: TrophyRepository,
    internal val goalRepo: GoalRepository,
    internal val settingsRepo: com.forge.app.data.prefs.SettingsRepository,
    internal val warmupRepo: com.forge.app.data.repo.WarmupRepository,
    internal val clock: Clock,
    @ApplicationContext internal val appContext: Context,
    internal val bridge: WorkoutSessionBridge,
    savedStateHandle: SavedStateHandle
) : ViewModel() {

    internal val dayKey: String = savedStateHandle.get<String>(Routes.ARG_DAY_KEY)
        ?: error("dayKey missing from SavedStateHandle")
    internal val dayPlan = Program.day(dayKey)
    internal val skipWarmup: Boolean = savedStateHandle.get<Boolean>(Routes.ARG_SKIP_WARMUP) ?: false

    internal val restTimer = RestTimerController(viewModelScope)

    internal val _state = MutableStateFlow(
        DayUiState(dayPlan = dayPlan, displayName = dayPlan.defaultName)
    )
    val state: StateFlow<DayUiState> = _state.asStateFlow()

    internal val _navigation = Channel<DayNavigationEffect>(Channel.BUFFERED)
    val navigationEffects = _navigation.receiveAsFlow()

    internal var undoClearJob: Job? = null

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
            val disabledUntilMs = settingsRepo.warmupDisabledUntilMs.firstOrNull() ?: 0L
            val warmupAutoSkipped = clock.nowMs() < disabledUntilMs
            _state.update {
                it.copy(
                    sessionId = sessionId,
                    sessionStartedAt = clock.nowMs(),
                    displayName = resolvedName,
                    isWarmupComplete = it.isWarmupComplete || skipWarmup || warmupAutoSkipped
                )
            }
            refreshExercises()
            startSessionService(resolvedName)
            workoutRepo.isNewSession(sessionId)
        }
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
            is DayUiEvent.ToggleExpanded, is DayUiEvent.LogSet, is DayUiEvent.LogSameAsLast,
            is DayUiEvent.DeleteSet, is DayUiEvent.EditSet, is DayUiEvent.RateExercise,
            is DayUiEvent.UpdateNote, is DayUiEvent.ToggleSkipped, is DayUiEvent.OpenGoalSetter,
            is DayUiEvent.SetGoal, is DayUiEvent.ClearGoal, is DayUiEvent.DismissGoalSetter,
            is DayUiEvent.SetExerciseUnit, is DayUiEvent.SetRestTimerOverride,
            is DayUiEvent.ConfirmWeightJump, is DayUiEvent.DismissWeightJump,
            is DayUiEvent.UpdateJournal, is DayUiEvent.SetPinnedNote,
            is DayUiEvent.ToggleSetDifficultyTag, is DayUiEvent.WarmupReaction,
            is DayUiEvent.MoveExercise, is DayUiEvent.LongPressExercise,
            is DayUiEvent.DismissQuickActions, is DayUiEvent.OpenAddExercisePicker,
            is DayUiEvent.CloseAddExercisePicker, is DayUiEvent.AddUnplannedExercise,
            is DayUiEvent.ToggleAmrap, is DayUiEvent.ToggleAssisted, is DayUiEvent.ToggleFailure,
            is DayUiEvent.SetSetType, is DayUiEvent.SetDropAnnotation, is DayUiEvent.SetRpe,
            is DayUiEvent.AddBonusSet, is DayUiEvent.SetUseKg,
            is DayUiEvent.SetSupersetGroup, is DayUiEvent.ShowWarmupSuggester,
            is DayUiEvent.ShowPlateCalculator, is DayUiEvent.DismissTrainingHelper,
            is DayUiEvent.LogBreak -> handleExerciseEvent(event)

            is DayUiEvent.OpenSwapPicker, is DayUiEvent.CloseSwapPicker,
            is DayUiEvent.PickSwapForSession, is DayUiEvent.PickSwapPersistent,
            is DayUiEvent.ClearPersistentSwap -> handleSwapEvent(event)

            is DayUiEvent.ToggleWarmupItem, is DayUiEvent.SkipWarmup,
            is DayUiEvent.DisableWarmupToday, is DayUiEvent.DisableWarmupWeek -> handleWarmupEvent(event)

            is DayUiEvent.RestTimerOpen, is DayUiEvent.RestTimerClose,
            is DayUiEvent.RestTimerPause, is DayUiEvent.RestTimerResume,
            is DayUiEvent.RestTimerReset, is DayUiEvent.RestTimerSkip,
            is DayUiEvent.RestTimerAddSeconds -> handleTimerEvent(event)

            is DayUiEvent.FinishWorkout, is DayUiEvent.DismissSummary,
            is DayUiEvent.RequestBack, is DayUiEvent.ConfirmDiscard,
            is DayUiEvent.DismissDiscardConfirm, is DayUiEvent.SaveAndExit,
            is DayUiEvent.UndoLastSet, is DayUiEvent.SetSessionType,
            is DayUiEvent.SetUntracked, is DayUiEvent.SetIntensity,
            is DayUiEvent.ConfirmPreSessionPicker -> handleSessionEvent(event)
        }
    }

    internal suspend fun refreshExercises() {
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
    internal suspend fun refreshExercise(exerciseId: String) {
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
    internal suspend fun refreshExerciseForSet(setId: Long) {
        val exId = findExerciseIdForSet(setId)
        if (exId != null) refreshExercise(exId) else refreshExercises()
    }

    internal fun findExerciseIdForSet(setId: Long): String? =
        _state.value.exercises.firstOrNull { ex -> ex.loggedSets.any { it.id == setId } }?.plan?.id

    internal suspend fun ensureLoggedExercise(exerciseId: String): Long? {
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

    internal fun findSet(setId: Long) =
        _state.value.exercises.flatMap { it.loggedSets }.firstOrNull { it.id == setId }

    internal fun startSessionService(dayName: String) {
        bridge.startSession(SessionNotifState(dayName, clock.nowMs()))
        appContext.startForegroundService(Intent(appContext, WorkoutSessionService::class.java))
    }

    internal fun stopSessionService() {
        bridge.endSession()
        appContext.stopService(Intent(appContext, WorkoutSessionService::class.java))
    }
}

sealed interface DayNavigationEffect {
    data object PopBack : DayNavigationEffect
}
