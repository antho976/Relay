package com.forge.app.ui.gym.train

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.ForgeHapticType
import com.forge.app.ui.common.forgeHaptic
import com.forge.app.ui.gym.train.components.AddExerciseSheet
import com.forge.app.ui.gym.train.components.PlateCalculatorDialog
import com.forge.app.ui.gym.train.components.RestTimerBubble
import com.forge.app.ui.gym.train.components.RestTimerControlsDialog
import com.forge.app.ui.gym.train.components.SessionSummarySheet
import com.forge.app.ui.gym.train.components.SwapPickerSheet
import com.forge.app.ui.gym.train.components.WarmupSuggesterDialog
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.DayUiState
import com.forge.app.ui.theme.LocalForgeSettings

@Suppress("UNUSED_PARAMETER")
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DayScreen(
    dayKey: String,
    onBack: () -> Unit,
    viewModel: DayViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }
    val hapticStrength = LocalForgeSettings.current.hapticStrength

    val view = LocalView.current
    val totalSets by remember { derivedStateOf { state.exercises.sumOf { it.loggedSets.size } } }
    val totalPrSets by remember { derivedStateOf { state.exercises.sumOf { it.prSetIds.size } } }
    var prevTotalSets = remember { mutableIntStateOf(-1) }
    var prevTotalPrs = remember { mutableIntStateOf(-1) }
    LaunchedEffect(totalSets, totalPrSets) {
        when {
            prevTotalPrs.intValue >= 0 && totalPrSets > prevTotalPrs.intValue ->
                view.forgeHaptic(ForgeHapticType.PR_OR_FINISH, hapticStrength)
            prevTotalSets.intValue >= 0 && totalSets > prevTotalSets.intValue ->
                view.forgeHaptic(ForgeHapticType.SET_LOGGED, hapticStrength)
        }
        prevTotalSets.intValue = totalSets
        prevTotalPrs.intValue = totalPrSets
    }

    LaunchedEffect(state.restTimer?.isFinished) {
        if (state.restTimer?.isFinished == true) view.forgeHaptic(ForgeHapticType.PR_OR_FINISH, hapticStrength)
    }
    LaunchedEffect(state.restTimer?.secondsRemaining) {
        if (state.restTimer?.secondsRemaining == 10) view.forgeHaptic(ForgeHapticType.COUNTDOWN_TICK, hapticStrength)
    }

    LaunchedEffect(state.undoableSetId) {
        val setId = state.undoableSetId ?: return@LaunchedEffect
        val result = snackbarHostState.showSnackbar(message = "Set logged", actionLabel = "Undo", duration = SnackbarDuration.Short)
        if (result == SnackbarResult.ActionPerformed) viewModel.onEvent(DayUiEvent.UndoLastSet)
    }

    BackHandler(enabled = !state.isFinished) { viewModel.onEvent(DayUiEvent.RequestBack) }

    LaunchedEffect(viewModel) {
        viewModel.navigationEffects.collect { effect ->
            when (effect) { DayNavigationEffect.PopBack -> onBack() }
        }
    }

    Scaffold(
        snackbarHost = { SnackbarHost(snackbarHostState) },
        floatingActionButton = {
            state.restTimer?.let { timer ->
                RestTimerBubble(state = timer,
                    onOpenControls = { viewModel.onEvent(DayUiEvent.RestTimerOpen) },
                    onLongClick = { viewModel.onEvent(DayUiEvent.RestTimerAddSeconds(30)) })
            }
        },
        containerColor = Color.Transparent
    ) { inner ->
        Box(Modifier.fillMaxSize().padding(inner)) {
            DayContent(state = state, onEvent = viewModel::onEvent)
        }
    }

    if (state.showDiscardConfirm) {
        DiscardDialog(
            onConfirm = { viewModel.onEvent(DayUiEvent.ConfirmDiscard) },
            onDismiss = { viewModel.onEvent(DayUiEvent.DismissDiscardConfirm) }
        )
    }

    val timer = state.restTimer
    if (state.showTimerControls && timer != null) {
        RestTimerControlsDialog(
            state = timer,
            onPause = { viewModel.onEvent(DayUiEvent.RestTimerPause) },
            onResume = { viewModel.onEvent(DayUiEvent.RestTimerResume) },
            onReset = { viewModel.onEvent(DayUiEvent.RestTimerReset) },
            onSkip = { viewModel.onEvent(DayUiEvent.RestTimerSkip) },
            onAddSeconds = { s -> viewModel.onEvent(DayUiEvent.RestTimerAddSeconds(s)) },
            onDismiss = { viewModel.onEvent(DayUiEvent.RestTimerClose) }
        )
    }

    state.swapPickerExercise?.let { exerciseUi ->
        SwapPickerSheet(
            forExercise = exerciseUi.plan,
            hasPersistentSwap = exerciseUi.persistentSwapName != null,
            currentSwapName = exerciseUi.sessionSwapName ?: exerciseUi.persistentSwapName,
            onPickForSession = { swap -> viewModel.onEvent(DayUiEvent.PickSwapForSession(exerciseUi.plan.id, swap)) },
            onPickPersistent = { swap -> viewModel.onEvent(DayUiEvent.PickSwapPersistent(exerciseUi.plan.id, swap)) },
            onClearPersistent = { viewModel.onEvent(DayUiEvent.ClearPersistentSwap(exerciseUi.plan.id)) },
            onDismiss = { viewModel.onEvent(DayUiEvent.CloseSwapPicker) }
        )
    }

    state.pendingWeightJumpWarning?.let { warning ->
        AlertDialog(
            onDismissRequest = { viewModel.onEvent(DayUiEvent.DismissWeightJump) },
            title = { Text("Big jump — are you sure?") },
            text = { Text("${warning.lastWeightLb.toInt()} lb → ${warning.newWeightLb.toInt()} lb is a ${((warning.newWeightLb / warning.lastWeightLb - 1) * 100).toInt()}% increase. Log it anyway?") },
            confirmButton = { Button(onClick = { viewModel.onEvent(DayUiEvent.ConfirmWeightJump) }) { Text("Log it") } },
            dismissButton = { TextButton(onClick = { viewModel.onEvent(DayUiEvent.DismissWeightJump) }) { Text("Go back") } }
        )
    }

    state.quickActionsForExerciseId?.let { exId ->
        val exercise = state.exercises.firstOrNull { it.plan.id == exId }
        AlertDialog(
            onDismissRequest = { viewModel.onEvent(DayUiEvent.DismissQuickActions) },
            title = { Text(exercise?.effectiveName ?: exId) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    listOf(
                        "Toggle skip" to { viewModel.onEvent(DayUiEvent.ToggleSkipped(exId)) },
                        "Open swap picker" to { viewModel.onEvent(DayUiEvent.OpenSwapPicker(exId)) },
                        "Set rest timer" to { viewModel.onEvent(DayUiEvent.DismissQuickActions) }
                    ).forEach { (label, action) ->
                        TextButton(onClick = { action(); viewModel.onEvent(DayUiEvent.DismissQuickActions) },
                            modifier = Modifier.fillMaxWidth()) { Text(label, modifier = Modifier.fillMaxWidth()) }
                    }
                }
            },
            confirmButton = {},
            dismissButton = { TextButton(onClick = { viewModel.onEvent(DayUiEvent.DismissQuickActions) }) { Text("Cancel") } }
        )
    }

    if (state.showAddExercisePicker) {
        AddExerciseSheet(
            alreadyAddedIds = state.exercises.map { it.plan.id }.toSet(),
            onPick = { exerciseId -> viewModel.onEvent(DayUiEvent.AddUnplannedExercise(exerciseId)) },
            onDismiss = { viewModel.onEvent(DayUiEvent.CloseAddExercisePicker) }
        )
    }

    state.goalSetterForExerciseId?.let { exerciseId ->
        val exercise = state.exercises.firstOrNull { it.plan.id == exerciseId }
        GoalSetterDialog(
            exerciseName = exercise?.effectiveName ?: exerciseId,
            currentGoal = exercise?.goalWeightLb,
            onSet = { lb -> viewModel.onEvent(DayUiEvent.SetGoal(exerciseId, lb)) },
            onClear = { viewModel.onEvent(DayUiEvent.ClearGoal(exerciseId)) },
            onDismiss = { viewModel.onEvent(DayUiEvent.DismissGoalSetter) }
        )
    }

    state.warmupSuggesterForExerciseId?.let { exerciseId ->
        val ex = state.exercises.firstOrNull { it.plan.id == exerciseId }
        val workingWeight = ex?.loggedSets?.lastOrNull()?.weightLb ?: ex?.prefillWeight?.toDoubleOrNull()
        WarmupSuggesterDialog(workingWeightLb = workingWeight, onDismiss = { viewModel.onEvent(DayUiEvent.DismissTrainingHelper) })
    }
    state.plateCalculatorForExerciseId?.let { exerciseId ->
        val ex = state.exercises.firstOrNull { it.plan.id == exerciseId }
        val workingWeight = ex?.loggedSets?.lastOrNull()?.weightLb ?: ex?.prefillWeight?.toDoubleOrNull()
        PlateCalculatorDialog(initialWeightLb = workingWeight, onDismiss = { viewModel.onEvent(DayUiEvent.DismissTrainingHelper) })
    }

    state.summary?.let { summary ->
        SessionSummarySheet(summary = summary, onDismiss = { mood, tags, journal ->
            viewModel.onEvent(DayUiEvent.DismissSummary(mood, tags))
            if (journal.isNotBlank()) viewModel.onEvent(DayUiEvent.UpdateJournal(journal))
        })
    }
}
