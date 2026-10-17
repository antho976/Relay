package com.forge.app.ui.gym.train

import com.forge.app.ui.gym.train.components.LocalInlineRestOnScreen
import androidx.compose.runtime.CompositionLocalProvider
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import com.forge.app.ui.common.ForgeSnackbar
import com.forge.app.ui.common.window.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
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
    // Held as a State and read only where the timer is drawn (FAB bubble, inline slot, controls):
    // reading it here would recompose the whole screen every second of every rest.
    val restTimer = viewModel.restTimerState.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }
    val hapticStrength = LocalForgeSettings.current.hapticStrength

    val view = LocalView.current
    val totalSets by remember { derivedStateOf { state.exercises.sumOf { it.loggedSets.size } } }
    val totalPrSets by remember { derivedStateOf { state.exercises.sumOf { it.prSetIds.size } } }
    // Arms only once the session has loaded, so a reopened session's existing PR sets are the
    // baseline rather than a replayed celebration (audit 2026-09-26); see SetLogHapticCues.
    val setLogCues = remember { SetLogHapticCues() }
    var showPrBurst by remember { mutableStateOf(false) }
    LaunchedEffect(state.isLoading, totalSets, totalPrSets) {
        val cue = setLogCues.advance(state.isLoading, totalSets, totalPrSets) ?: return@LaunchedEffect
        view.forgeHaptic(cue, hapticStrength)
        if (cue == ForgeHapticType.PR_OR_FINISH) {
            // A PR plays confetti + leaves the gold set-row text; the full-screen takeover was removed.
            showPrBurst = true
            // A4: announce to TalkBack (phone is often face-down mid-set). No-op without a screen reader.
            view.announceForAccessibility("New personal record")
        }
    }

    // Keep the screen awake while a session is in progress so the phone doesn't lock mid-rest and
    // force a PIN/biometric unlock between sets. Gated on the Session setting (GYMAP-74, default on);
    // released the moment the session finishes, the setting turns off, or the screen leaves composition.
    val keepScreenOn = LocalForgeSettings.current.keepScreenOn
    DisposableEffect(state.isFinished, keepScreenOn) {
        view.keepScreenOn = keepScreenOn && !state.isFinished
        onDispose { view.keepScreenOn = false }
    }

    // The rest timer's two cues (the warning at 0:10, then "rest complete") fire from HERE and
    // nowhere else: this is the owner that reads the Feedback strength setting, so Off plays
    // nothing, and one owner means one pulse per threshold. The bubble draws these moments; it does
    // not buzz them. Seeding the tracker with the current reading means a screen rebuilt mid-rest
    // (rotation) or opened onto an already-finished timer never re-buzzes / re-announces (A4).
    val restCues = remember {
        RestTimerHapticCues(restTimer.value?.secondsRemaining, restTimer.value?.isFinished == true)
    }
    val currentHapticStrength by rememberUpdatedState(hapticStrength)
    LaunchedEffect(restCues) {
        // Collected in the effect rather than keyed on the reading, so the ticks never pass through
        // this screen's composition. Same inputs as before: advance on each change of (seconds, done).
        snapshotFlow { restTimer.value?.secondsRemaining to (restTimer.value?.isFinished == true) }
            .distinctUntilChanged()
            .collect { (seconds, finished) ->
                restCues.advance(seconds, finished).forEach { cue ->
                    view.forgeHaptic(cue, currentHapticStrength)
                    // A4: spoken even with the phone face-down.
                    if (cue == ForgeHapticType.PR_OR_FINISH) view.announceForAccessibility("Rest complete")
                }
            }
    }

    LaunchedEffect(state.deletedSet) {
        state.deletedSet ?: return@LaunchedEffect
        // Newest wins: a still-showing "Set logged" would otherwise queue this one behind it.
        snackbarHostState.currentSnackbarData?.dismiss()
        // Long, not Short: four seconds ran out on the emulator before a deliberate tap landed, and
        // a lifter who swiped by accident is often looking at the bar, not the phone.
        val result = snackbarHostState.showSnackbar(message = "Set deleted", actionLabel = "Undo", duration = SnackbarDuration.Long)
        viewModel.onEvent(
            if (result == SnackbarResult.ActionPerformed) DayUiEvent.RestoreDeletedSet else DayUiEvent.ForgetDeletedSet
        )
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

    val inlineRestOnScreen = remember { mutableStateOf(false) }
    Scaffold(
        snackbarHost = { SnackbarHost(snackbarHostState) { ForgeSnackbar(it) } },
        floatingActionButton = {
            // Retain the last timer so the bubble can animate OUT (scale + fade) when the rest
            // ends instead of snapping away. Enter is handled by the bubble's own pop-in spring.
            val timer = restTimer.value
            var lastTimer by remember { mutableStateOf(timer) }
            LaunchedEffect(timer) { if (timer != null) lastTimer = timer }
            // The list doesn't make room for the keyboard (edge-to-edge, no IME padding), so the
            // card's rest row can sit under it while still counting as on screen. Typing the next
            // set is exactly when that happens, so an open keyboard always gets the bubble.
            val imeOpen = WindowInsets.ime.getBottom(LocalDensity.current) > 0
            AnimatedVisibility(
                // Only once the card's own rest row is off screen: with both showing, the same
                // countdown appeared twice and the bubble covered the card's action row.
                visible = timer != null && (!inlineRestOnScreen.value || imeOpen),
                enter = fadeIn(ForgeMotion.enterTween(ForgeMotion.DurationFast)),
                exit = scaleOut(ForgeMotion.exitTween(), targetScale = 0.6f) + fadeOut(ForgeMotion.exitTween())
            ) {
                (timer ?: lastTimer)?.let { t ->
                    RestTimerBubble(state = t,
                        onOpenControls = { viewModel.onEvent(DayUiEvent.RestTimerOpen) },
                        onLongClick = { viewModel.onEvent(DayUiEvent.RestTimerAddSeconds(30)) },
                        // Edge-to-edge, the Scaffold's FAB slot sits under an open keyboard; ride
                        // above it instead of vanishing while the next set's numbers are typed.
                        modifier = Modifier.imePadding())
                }
            }
        },
        containerColor = Color.Transparent
    ) { inner ->
        Box(Modifier.fillMaxSize().padding(inner)) {
            CompositionLocalProvider(LocalInlineRestOnScreen provides inlineRestOnScreen) {
                DayContent(state = state, onEvent = viewModel::onEvent, restTimer = restTimer)
            }
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
            onKeepGoing = { viewModel.onEvent(DayUiEvent.DismissDiscardConfirm) },
            // Back only asks when there is work to keep; an empty FINISH asks too, with nothing to resume.
            nothingLogged = !state.hasUnsavedWork
        )
    }

    if (state.finishUntouched.isNotEmpty()) {
        FinishUntouchedDialog(
            untouched = state.finishUntouched,
            onFinish = { viewModel.onEvent(DayUiEvent.ConfirmFinishUntouched) },
            onKeepGoing = { viewModel.onEvent(DayUiEvent.DismissFinishUntouched) }
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

    // Read only while the dialog is open, so a closed dialog adds no per-tick recomposition.
    val timer = if (state.showTimerControls) restTimer.value else null
    if (timer != null) {
        RestTimerControlsDialog(
            state = timer,
            onPause = { viewModel.onEvent(DayUiEvent.RestTimerPause) },
            onResume = { viewModel.onEvent(DayUiEvent.RestTimerResume) },
            onReset = { viewModel.onEvent(DayUiEvent.RestTimerReset) },
            onSkip = { viewModel.onEvent(DayUiEvent.RestTimerSkip) },
            onAddSeconds = { s -> viewModel.onEvent(DayUiEvent.RestTimerAddSeconds(s)) },
            onDismiss = { viewModel.onEvent(DayUiEvent.RestTimerClose) },
            reason = state.restTimerReason,
            onSetSeconds = { s -> viewModel.onEvent(DayUiEvent.RestTimerSetTo(s)) },
            onMakeDefault = state.restTimerExerciseId?.let { id ->
                { viewModel.onEvent(DayUiEvent.SetRestTimerOverride(id, timer.totalSeconds)) }
            },
            exerciseName = state.restTimerExerciseId?.let { id -> state.exercises.firstOrNull { it.plan.id == id }?.effectiveName },
            onClearDefault = state.restTimerExerciseId
                ?.takeIf { id -> state.exercises.firstOrNull { it.plan.id == id }?.restTimerOverrideSeconds != null }
                ?.let { id -> { viewModel.onEvent(DayUiEvent.SetRestTimerOverride(id, null)) } }
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
            title = { Text("Big jump") },
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
                    //
                    // The warm-up ladder, plate math and the goal setter were built and wired to
                    // events nothing raised, so FEATURES.md listed a plate calculator and in-workout
                    // goals nobody could open. They live here, shown only where they mean something.
                    val unit = exercise?.effectiveUnit
                    val loadBearing = unit == com.forge.app.program.ExerciseUnit.WEIGHT ||
                        unit == com.forge.app.program.ExerciseUnit.DUMBBELL ||
                        unit == com.forge.app.program.ExerciseUnit.PLATES
                    val barLoaded = exercise != null && unit == com.forge.app.program.ExerciseUnit.WEIGHT &&
                        (com.forge.app.program.ExerciseLibrary.byId(exercise.effectiveExerciseId)?.equipment ?: exercise.plan.equipment)
                            .any { it == com.forge.app.program.Equipment.BARBELL || it == com.forge.app.program.Equipment.TRAP_BAR }
                    buildList {
                        if (loadBearing) add("Warm-up sets" to { viewModel.onEvent(DayUiEvent.ShowWarmupSuggester(exId)) })
                        if (barLoaded) add("Plate math" to { viewModel.onEvent(DayUiEvent.ShowPlateCalculator(exId)) })
                        if (loadBearing && unit != com.forge.app.program.ExerciseUnit.PLATES) add(
                            (if (exercise?.goalWeightLb != null) "Change goal" else "Set a goal") to
                                { viewModel.onEvent(DayUiEvent.OpenGoalSetter(exId)) }
                        )
                        add("Swap exercise" to { viewModel.onEvent(DayUiEvent.OpenSwapPicker(exId)) })
                        add((if (exercise?.skipped == true) "Unskip exercise" else "Skip exercise") to
                            { viewModel.onEvent(DayUiEvent.ToggleSkipped(exId)) })
                    }.forEach { (label, action) ->
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
            onDismiss = { viewModel.onEvent(DayUiEvent.DismissTrainingHelper) },
            barLoadLb = ex?.let {
                com.forge.app.domain.warmup.WarmupEngine.barLoadFor(
                    it.effectiveExerciseId, it.effectiveUnit, LocalForgeSettings.current.weightUnit.isMetric
                )
            }
        )
    }
    state.plateCalculatorForExerciseId?.let { exerciseId ->
        val ex = state.exercises.firstOrNull { it.plan.id == exerciseId }
        // In pounds. prefillWeight is display-unit text (kg, stones, or a plate count), and passing
        // it as lb seeded a 100 kg lift as 45.4 kg. Today's last set, else the coach's target, else
        // last session's first set.
        val workingWeight = ex?.loggedSets?.lastOrNull()?.weightLb
            ?: ex?.suggestedTargetLb
            ?: ex?.priorSets?.firstOrNull()?.weightLb
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
