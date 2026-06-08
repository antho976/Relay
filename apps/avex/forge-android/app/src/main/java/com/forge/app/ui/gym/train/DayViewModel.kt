package com.forge.app.ui.gym.train

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.core.time.Clock
import com.forge.app.data.repo.CustomizationRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.data.repo.TrophyRepository
import com.forge.app.data.repo.WorkoutRepository
import com.forge.app.domain.timer.RestTimerController
import com.forge.app.program.Equipment
import com.forge.app.program.Program
import com.forge.app.service.WorkoutSessionBridge
import com.forge.app.ui.gym.train.state.CrossDaySessionInfo
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.DayUiState
import com.forge.app.ui.nav.Routes
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import android.content.Context
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
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

    internal val restTimer = RestTimerController(viewModelScope, clock)

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
            // Only one session is active at a time. If a *different* day's workout is still in
            // progress, prompt (resume it, or discard & start this one) instead of silently
            // resuming the wrong day's sets under this plan.
            val active = workoutRepo.activeSession()
            if (active != null && active.dayKey != dayKey) {
                val otherName = customizationRepo.getDayName(active.dayKey)?.customName
                    ?: Program.days.firstOrNull { it.key == active.dayKey }?.defaultName
                    ?: active.dayKey
                _state.update { it.copy(isLoading = false, crossDaySession = CrossDaySessionInfo(active.dayKey, otherName)) }
                return@launch
            }
            beginSessionForThisDay()
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
        // Equipment + dislikes drive the swap picker's candidate pool (program-unlock Phase 4).
        viewModelScope.launch {
            settingsRepo.availableEquipment
                .combine(settingsRepo.dislikedExercises) { equip, disliked -> equip to disliked }
                .collect { (equip, disliked) ->
                    val available = equip.mapNotNull {
                        runCatching { Equipment.valueOf(it) }.getOrNull()
                    }.toSet()
                    _state.update { it.copy(swapAvailableEquipment = available, swapDislikedIds = disliked) }
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
            is DayUiEvent.LeaveAndResume, is DayUiEvent.CrossDayDiscardAndStart,
            is DayUiEvent.CrossDayGoBack,
            is DayUiEvent.UndoLastSet, is DayUiEvent.SetSessionType,
            is DayUiEvent.SetUntracked, is DayUiEvent.SetIntensity,
            is DayUiEvent.ConfirmPreSessionPicker -> handleSessionEvent(event)
        }
    }
}

sealed interface DayNavigationEffect {
    data object PopBack : DayNavigationEffect
}
