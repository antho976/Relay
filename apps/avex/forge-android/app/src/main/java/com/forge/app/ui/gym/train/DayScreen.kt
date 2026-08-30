package com.forge.app.ui.gym.train

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleOut
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
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.ConfettiOverlay
import com.forge.app.ui.common.ForgeHapticType
import com.forge.app.ui.common.forgeHaptic
import com.forge.app.ui.gym.train.components.AddExerciseSheet
import com.forge.app.ui.gym.train.components.DislikeSwapPromptDialog
import com.forge.app.ui.gym.train.components.PlateCalculatorDialog
import com.forge.app.ui.gym.train.components.RestTimerBubble
import com.forge.app.ui.gym.train.components.RestTimerControlsDialog
import com.forge.app.ui.gym.train.components.SessionSummarySheet
import com.forge.app.ui.gym.train.components.SwapPickerSheet
import com.forge.app.ui.gym.train.components.WarmupSuggesterDialog
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.DayUiState
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings

// DEPRECATION: View.announceForAccessibility (A4) is deprecated as of API 36 but is still the
// reliable one-shot screen-reader announce; the suggested live-region replacement needs a persistent
// hidden node and is far more fragile, so we keep this until a clean Compose API exists.
@Suppress("UNUSED_PARAMETER", "DEPRECATION")
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
    var showPrBurst by remember { mutableStateOf(false) }
    LaunchedEffect(totalSets, totalPrSets) {
        when {
            prevTotalPrs.intValue >= 0 && totalPrSets > prevTotalPrs.intValue -> {
                view.forgeHaptic(ForgeHapticType.PR_OR_FINISH, hapticStrength)
                // A PR plays confetti + leaves the gold set-row text; the full-screen takeover was removed.
                showPrBurst = true
                // A4: announce to TalkBack (phone is often face-down mid-set). No-op without a screen reader.
                view.announceForAccessibility("New personal record!")
            }
            prevTotalSets.intValue >= 0 && totalSets > prevTotalSets.intValue ->
                view.forgeHaptic(ForgeHapticType.SET_LOGGED, hapticStrength)
        }
        prevTotalSets.intValue = totalSets
        prevTotalPrs.intValue = totalPrSets
    }

    // Keep the screen awake while a session is in progress so the phone doesn't lock mid-rest and
    // force a PIN/biometric unlock between sets. Gated on the Session setting (GYMAP-74, default on);
    // released the moment the session finishes, the setting turns off, or the screen leaves composition.
    val keepScreenOn = LocalForgeSettings.current.keepScreenOn
    DisposableEffect(state.isFinished, keepScreenOn) {
        view.keepScreenOn = keepScreenOn && !state.isFinished
        onDispose { view.keepScreenOn = false }
    }

    // Fire only on a genuine not-finished → finished transition. Seeding prev with the current value
    // means re-entering the screen with an already-finished timer doesn't re-buzz / re-announce (A4).
    var prevRestFinished by remember { mutableStateOf(state.restTimer?.isFinished == true) }
    LaunchedEffect(state.restTimer?.isFinished) {
        val finished = state.restTimer?.isFinished == true
        if (finished && !prevRestFinished) {
            view.forgeHaptic(ForgeHapticType.PR_OR_FINISH, hapticStrength)
            view.announceForAccessibility("Rest complete") // A4: spoken even with the phone face-down.
        }
        prevRestFinished = finished
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

    LaunchedEffect(viewModel) {
        viewModel.messages.collect { msg -> snackbarHostState.showSnackbar(msg) }
    }

    Scaffold(
        snackbarHost = { SnackbarHost(snackbarHostState) },
        floatingActionButton = {
            // Retain the last timer so the bubble can animate OUT (scale + fade) when the rest
            // ends instead of snapping away. Enter is handled by the bubble's own pop-in spring.
            val timer = state.restTimer
            var lastTimer by remember { mutableStateOf(timer) }
            LaunchedEffect(timer) { if (timer != null) lastTimer = timer }
            AnimatedVisibility(
                visible = timer != null,
                enter = fadeIn(ForgeMotion.enterTween(ForgeMotion.DurationFast)),
                exit = scaleOut(ForgeMotion.exitTween(), targetScale = 0.6f) + fadeOut(ForgeMotion.exitTween())
            ) {
                (timer ?: lastTimer)?.let { t ->
                    RestTimerBubble(state = t,
                        onOpenControls = { viewModel.onEvent(DayUiEvent.RestTimerOpen) },
                        onLongClick = { viewModel.onEvent(DayUiEvent.RestTimerAddSeconds(30)) })
                }
            }
        },
        containerColor = Color.Transparent
    ) { inner ->
        Box(Modifier.fillMaxSize().padding(inner)) {
            DayContent(state = state, onEvent = viewModel::onEvent)
            if (showPrBurst) {
                // PR celebration is now just the confetti burst (over the live screen) plus the gold
                // ★/text on the record set row — no full-screen "PERSONAL RECORD" takeover.
                ConfettiOverlay(modifier = Modifier.fillMaxSize(), onComplete = { showPrBurst = false })
            }
        }
    }

    if (state.showDiscardConfirm) {
        LeaveSessionDialog(
            onResumeLater = { viewModel.onEvent(DayUiEvent.LeaveAndResume) },
            onDiscard = { viewModel.onEvent(DayUiEvent.ConfirmDiscard) },
            onKeepGoing = { viewModel.onEvent(DayUiEvent.DismissDiscardConfirm) }
        )
    }

    state.crossDaySession?.let { cross ->
        CrossDaySessionDialog(
            otherDayName = cross.dayName,
            thisDayName = state.displayName,
            onDiscardAndStart = { viewModel.onEvent(DayUiEvent.CrossDayDiscardAndStart) },
            onGoBack = { viewModel.onEvent(DayUiEvent.CrossDayGoBack) }
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
            onDismiss = { viewModel.onEvent(DayUiEvent.RestTimerClose) },
            reason = state.restTimerReason
        )
    }

    state.swapPickerExercise?.let { exerciseUi ->
        // Don't offer an exercise the day already has — picking it would put the same movement in the
        // day twice (a duplicate card that breaks per-exercise logging). Excludes every other slot's
        // exercise AND the one being swapped (it's the card title, no need to list it again).
        val alreadyInDay = state.exercises.map { it.effectiveName }.toSet()
        val swapCandidates = com.forge.app.program.ExerciseLibrary.swapCandidates(
            muscle = exerciseUi.plan.muscle,
            available = state.swapAvailableEquipment,
            disliked = state.swapDislikedIds,
            frozenIds = state.swapFrozenIds
        ).filterNot { it.name in alreadyInDay }
        SwapPickerSheet(
            forExercise = exerciseUi.plan,
            candidates = swapCandidates,
            hasPersistentSwap = exerciseUi.persistentSwapName != null,
            currentSwapName = exerciseUi.sessionSwapName ?: exerciseUi.persistentSwapName,
            onPickForSession = { swap -> viewModel.onEvent(DayUiEvent.PickSwapForSession(exerciseUi.plan.id, swap)) },
            onPickPersistent = { swap -> viewModel.onEvent(DayUiEvent.PickSwapPersistent(exerciseUi.plan.id, swap)) },
            onClearPersistent = { viewModel.onEvent(DayUiEvent.ClearPersistentSwap(exerciseUi.plan.id)) },
            onDismiss = { viewModel.onEvent(DayUiEvent.CloseSwapPicker) }
        )
    }

    state.dislikeSwapPrompt?.let { prompt ->
        DislikeSwapPromptDialog(
            exerciseName = prompt.exerciseName,
            onDislike = { viewModel.onEvent(DayUiEvent.DislikeSwappedExercise) },
            onKeep = { viewModel.onEvent(DayUiEvent.DismissDislikePrompt) },
            onNotThisWorkout = { viewModel.onEvent(DayUiEvent.SuppressDislikePromptThisSession) },
            onNeverAsk = { viewModel.onEvent(DayUiEvent.NeverAskDislikePrompt) }
        )
    }

    state.pendingWeightJumpWarning?.let { warning ->
        AlertDialog(
            onDismissRequest = { viewModel.onEvent(DayUiEvent.DismissWeightJump) },
            title = { Text("Big jump — are you sure?") },
            text = { Text("${warning.lastLabel} → ${warning.newLabel} is a ${warning.percent}% increase. Log it anyway?") },
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
                    // "Set rest timer" is gone rather than wired: its action was
                    // DismissQuickActions, i.e. a menu row whose entire effect was to close the
                    // menu, sitting between two rows that do something. The exercise card already
                    // exposes the timer setter directly, so the row was a second, non-working route
                    // to a control one tap away — and the menu is shorter without it.
                    listOf(
                        "Toggle skip" to { viewModel.onEvent(DayUiEvent.ToggleSkipped(exId)) },
                        "Open swap picker" to { viewModel.onEvent(DayUiEvent.OpenSwapPicker(exId)) }
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
        val workingWeight = ex?.suggestedTargetLb
            ?: ex?.loggedSets?.lastOrNull()?.weightLb
            ?: ex?.priorSets?.mapNotNull { it.weightLb }?.maxOrNull()
        // "Already warm" = an earlier exercise for the same muscle has sets on the board. That
        // collapses the ramp to at most one feeler set instead of a full ladder (WarmupEngine).
        val alreadyWarm = ex != null && state.exercises
            .takeWhile { it.plan.id != exerciseId }
            .any { it.plan.muscle == ex.plan.muscle && it.loggedSets.isNotEmpty() }
        WarmupSuggesterDialog(
            exerciseName = ex?.effectiveName ?: "",
            unit = ex?.effectiveUnit ?: com.forge.app.program.ExerciseUnit.WEIGHT,
            isCompound = ex?.plan?.let { com.forge.app.program.SessionEstimate.isCompound(it) } ?: true,
            targetReps = ex?.plan?.reps?.let { r ->
                Regex("\\d+").findAll(r).mapNotNull { it.value.toIntOrNull() }.minOrNull()
            } ?: 10,
            muscleAlreadyWarm = alreadyWarm,
            workingWeightLb = workingWeight,
            weightUnit = LocalForgeSettings.current.weightUnit,
            onDismiss = { viewModel.onEvent(DayUiEvent.DismissTrainingHelper) }
        )
    }
    state.plateCalculatorForExerciseId?.let { exerciseId ->
        val ex = state.exercises.firstOrNull { it.plan.id == exerciseId }
        val workingWeight = ex?.loggedSets?.lastOrNull()?.weightLb ?: ex?.prefillWeight?.toDoubleOrNull()
        PlateCalculatorDialog(initialWeightLb = workingWeight, weightUnit = LocalForgeSettings.current.weightUnit,
            onDismiss = { viewModel.onEvent(DayUiEvent.DismissTrainingHelper) })
    }

    state.summary?.let { summary ->
        SessionSummarySheet(summary = summary, onDismiss = { mood, tags, journal ->
            // Fold the journal into DismissSummary so it's written in the same coroutine that runs
            // before PopBack — a separate UpdateJournal event raced the VM clearing and could be lost.
            viewModel.onEvent(DayUiEvent.DismissSummary(mood, tags, journal))
        })
    }
}
