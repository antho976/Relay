package com.forge.app.ui.gym.train

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.foundation.clickable
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.foundation.border
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.sp
import android.view.HapticFeedbackConstants
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.FilterChip
import androidx.compose.material3.OutlinedTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.foundation.background
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.gym.train.components.ExerciseCard
import com.forge.app.ui.gym.train.components.PlateCalculatorDialog
import com.forge.app.ui.gym.train.components.RestTimerBubble
import com.forge.app.ui.gym.train.components.WarmupGate
import com.forge.app.ui.gym.train.components.WarmupSuggesterDialog
import com.forge.app.ui.gym.train.components.RestTimerControlsDialog
import com.forge.app.ui.gym.train.components.SessionSummarySheet
import com.forge.app.ui.gym.train.components.SwapPickerSheet
import com.forge.app.domain.units.parseToLb
import com.forge.app.ui.gym.train.components.AddExerciseSheet
import com.forge.app.ui.common.ForgeHapticType
import com.forge.app.ui.common.forgeHaptic
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.DayUiState
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Suppress("UNUSED_PARAMETER") // dayKey arrives via SavedStateHandle; not used here
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

    // ── Haptics (#26 set logged, #50 PR, #51 10s countdown) ──────────────────
    val view = LocalView.current
    val totalSets by remember { derivedStateOf { state.exercises.sumOf { it.loggedSets.size } } }
    val totalPrSets by remember { derivedStateOf { state.exercises.sumOf { it.prSetIds.size } } }
    var prevTotalSets by remember { mutableIntStateOf(-1) }
    var prevTotalPrs by remember { mutableIntStateOf(-1) }
    LaunchedEffect(totalSets, totalPrSets) {
        when {
            prevTotalPrs >= 0 && totalPrSets > prevTotalPrs -> {
                view.forgeHaptic(ForgeHapticType.PR_OR_FINISH, hapticStrength)
            }
            prevTotalSets >= 0 && totalSets > prevTotalSets ->
                view.forgeHaptic(ForgeHapticType.SET_LOGGED, hapticStrength)
        }
        prevTotalSets = totalSets
        prevTotalPrs = totalPrSets
    }

    LaunchedEffect(state.restTimer?.isFinished) {
        if (state.restTimer?.isFinished == true)
            view.forgeHaptic(ForgeHapticType.PR_OR_FINISH, hapticStrength)
    }

    LaunchedEffect(state.restTimer?.secondsRemaining) {
        if (state.restTimer?.secondsRemaining == 10)
            view.forgeHaptic(ForgeHapticType.COUNTDOWN_TICK, hapticStrength)
    }

    // ── Undo snackbar (#46) ───────────────────────────────────────────────────
    LaunchedEffect(state.undoableSetId) {
        val setId = state.undoableSetId ?: return@LaunchedEffect
        val result = snackbarHostState.showSnackbar(
            message = "Set logged",
            actionLabel = "Undo",
            duration = SnackbarDuration.Short
        )
        if (result == SnackbarResult.ActionPerformed) {
            viewModel.onEvent(DayUiEvent.UndoLastSet)
        }
    }

    BackHandler(enabled = !state.isFinished) {
        viewModel.onEvent(DayUiEvent.RequestBack)
    }

    LaunchedEffect(viewModel) {
        viewModel.navigationEffects.collect { effect ->
            when (effect) {
                DayNavigationEffect.PopBack -> onBack()
            }
        }
    }

    Scaffold(
        snackbarHost = { SnackbarHost(snackbarHostState) },
        floatingActionButton = {
            state.restTimer?.let { timer ->
                Column(horizontalAlignment = Alignment.End) {
                    // "Next up" label above the timer bubble (#99)
                    state.nextUpExerciseName?.let { name ->
                        Text(
                            "next: $name",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.65f),
                            maxLines = 1,
                            modifier = Modifier.padding(bottom = 6.dp)
                        )
                    }
                    RestTimerBubble(
                        state = timer,
                        onOpenControls = { viewModel.onEvent(DayUiEvent.RestTimerOpen) },
                        onLongClick = { viewModel.onEvent(DayUiEvent.RestTimerAddSeconds(30)) }
                    )
                }
            }
        },
        containerColor = Color.Transparent
    ) { inner ->
        Box(Modifier.fillMaxSize().padding(inner)) {
            DayContent(
                state = state,
                onEvent = viewModel::onEvent
            )
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
            onPickForSession = { swap ->
                viewModel.onEvent(DayUiEvent.PickSwapForSession(exerciseUi.plan.id, swap))
            },
            onPickPersistent = { swap ->
                viewModel.onEvent(DayUiEvent.PickSwapPersistent(exerciseUi.plan.id, swap))
            },
            onClearPersistent = {
                viewModel.onEvent(DayUiEvent.ClearPersistentSwap(exerciseUi.plan.id))
            },
            onDismiss = { viewModel.onEvent(DayUiEvent.CloseSwapPicker) }
        )
    }

    state.pendingWeightJumpWarning?.let { warning ->
        AlertDialog(
            onDismissRequest = { viewModel.onEvent(DayUiEvent.DismissWeightJump) },
            title = { Text("Big jump — are you sure?") },
            text = { Text("${warning.lastWeightLb.toInt()} lb → ${warning.newWeightLb.toInt()} lb is a ${((warning.newWeightLb / warning.lastWeightLb - 1) * 100).toInt()}% increase. Log it anyway?") },
            confirmButton = {
                Button(onClick = { viewModel.onEvent(DayUiEvent.ConfirmWeightJump) }) { Text("Log it") }
            },
            dismissButton = {
                TextButton(onClick = { viewModel.onEvent(DayUiEvent.DismissWeightJump) }) { Text("Go back") }
            }
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
                        "Set rest timer" to { viewModel.onEvent(DayUiEvent.DismissQuickActions); /* handled via DayContent local state */ }
                    ).forEach { (label, action) ->
                        TextButton(
                            onClick = { action(); viewModel.onEvent(DayUiEvent.DismissQuickActions) },
                            modifier = Modifier.fillMaxWidth()
                        ) { Text(label, modifier = Modifier.fillMaxWidth()) }
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

    // Training helper dialogs (#10, #11)
    state.warmupSuggesterForExerciseId?.let { exerciseId ->
        val ex = state.exercises.firstOrNull { it.plan.id == exerciseId }
        val workingWeight = ex?.loggedSets?.lastOrNull()?.weightLb
            ?: ex?.prefillWeight?.toDoubleOrNull()
        WarmupSuggesterDialog(
            workingWeightLb = workingWeight,
            onDismiss = { viewModel.onEvent(DayUiEvent.DismissTrainingHelper) }
        )
    }
    state.plateCalculatorForExerciseId?.let { exerciseId ->
        val ex = state.exercises.firstOrNull { it.plan.id == exerciseId }
        val workingWeight = ex?.loggedSets?.lastOrNull()?.weightLb
            ?: ex?.prefillWeight?.toDoubleOrNull()
        PlateCalculatorDialog(
            initialWeightLb = workingWeight,
            onDismiss = { viewModel.onEvent(DayUiEvent.DismissTrainingHelper) }
        )
    }

    state.summary?.let { summary ->
        SessionSummarySheet(
            summary = summary,
            onDismiss = { mood, tags, journal ->
                viewModel.onEvent(DayUiEvent.DismissSummary(mood, tags))
                if (journal.isNotBlank()) viewModel.onEvent(DayUiEvent.UpdateJournal(journal))
            }
        )
    }

}

@Composable
private fun DayContent(
    state: DayUiState,
    onEvent: (DayUiEvent) -> Unit
) {
    if (state.isLoading) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text("Loading session…", color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        return
    }

    var restTimerSetterForId by remember { mutableStateOf<String?>(null) }

    restTimerSetterForId?.let { exId ->
        val exercise = state.exercises.firstOrNull { it.plan.id == exId }
        RestTimerSetterDialog(
            exerciseName = exercise?.effectiveName ?: exId,
            currentSeconds = exercise?.restTimerOverrideSeconds,
            onSet = { secs -> onEvent(DayUiEvent.SetRestTimerOverride(exId, secs)); restTimerSetterForId = null },
            onClear = { onEvent(DayUiEvent.SetRestTimerOverride(exId, null)); restTimerSetterForId = null },
            onDismiss = { restTimerSetterForId = null }
        )
    }

    val useKg = LocalForgeSettings.current.useKg

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(vertical = 8.dp)
    ) {
        if (!state.isWarmupComplete) {
            item(key = "warmup") {
                WarmupGate(
                    warmupItems = state.customWarmupItems ?: state.dayPlan.warmup,
                    checks = state.warmupChecks,
                    onToggle = { idx -> onEvent(DayUiEvent.ToggleWarmupItem(idx)) },
                    onSkip = { onEvent(DayUiEvent.SkipWarmup) },
                    onDisableToday = { onEvent(DayUiEvent.DisableWarmupToday) },
                    onDisableWeek = { onEvent(DayUiEvent.DisableWarmupWeek) }
                )
            }
        } else {
            item(key = "session-hero") {
                SessionHero(
                    state = state,
                    onBack = { onEvent(DayUiEvent.RequestBack) },
                    onFinish = { onEvent(DayUiEvent.FinishWorkout) }
                )
            }
            val nowExerciseId = state.exercises.firstOrNull {
                !it.skipped && it.loggedSets.size < it.plan.sets
            }?.plan?.id
            items(state.exercises, key = { it.plan.id }) { exerciseState ->
                val idx = state.exercises.indexOf(exerciseState)
                ExerciseCard(
                    exerciseIndex = idx,
                    state = exerciseState,
                    isNow = exerciseState.plan.id == nowExerciseId,
                    onToggle = { onEvent(DayUiEvent.ToggleExpanded(exerciseState.plan.id)) },
                    onLogSet = { weight, reps ->
                        // Convert kg input to lb storage if unit is kg
                        val storedWeight = if (useKg) {
                            val lb = parseToLb(weight, useKg = true)
                            if (lb != null) "%.1f".format(lb).trimEnd('0').trimEnd('.') else weight
                        } else weight
                        onEvent(DayUiEvent.LogSet(exerciseState.plan.id, storedWeight, reps))
                    },
                    onDeleteSet = { setId -> onEvent(DayUiEvent.DeleteSet(setId)) },
                    onEditSet = { setId, w, r -> onEvent(DayUiEvent.EditSet(setId, w, r)) },
                    onLogSameAsLast = { setId ->
                        onEvent(DayUiEvent.LogSameAsLast(exerciseState.plan.id, setId))
                    },
                    onRate = { rating ->
                        onEvent(DayUiEvent.RateExercise(exerciseState.plan.id, rating))
                    },
                    onNoteChange = { note ->
                        onEvent(DayUiEvent.UpdateNote(exerciseState.plan.id, note))
                    },
                    onToggleSkipped = { onEvent(DayUiEvent.ToggleSkipped(exerciseState.plan.id)) },
                    onOpenSwapPicker = { onEvent(DayUiEvent.OpenSwapPicker(exerciseState.plan.id)) },
                    onOpenGoalSetter = { onEvent(DayUiEvent.OpenGoalSetter(exerciseState.plan.id)) },
                    onMoveUp = if (idx > 0) { { onEvent(DayUiEvent.MoveExercise(exerciseState.plan.id, -1)) } } else null,
                    onMoveDown = if (idx < state.exercises.size - 1) { { onEvent(DayUiEvent.MoveExercise(exerciseState.plan.id, 1)) } } else null,
                    onLongPress = { onEvent(DayUiEvent.LongPressExercise(exerciseState.plan.id)) },
                    onOpenRestTimerSetter = { restTimerSetterForId = exerciseState.plan.id },
                    onSetExerciseUnit = { unit -> onEvent(DayUiEvent.SetExerciseUnit(exerciseState.plan.id, unit)) },
                    onPinNote = { note -> onEvent(DayUiEvent.SetPinnedNote(exerciseState.plan.id, note)) },
                    onToggleSetDifficultyTag = { setId, tag -> onEvent(DayUiEvent.ToggleSetDifficultyTag(setId, tag)) }
                )
            }
            item(key = "add-exercise") {
                Text(
                    "+ add exercise",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { onEvent(DayUiEvent.OpenAddExercisePicker) }
                        .padding(horizontal = 24.dp, vertical = 16.dp)
                )
            }
        }
    }
}

@Composable
private fun DiscardDialog(
    onConfirm: () -> Unit,
    onDismiss: () -> Unit
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Discard workout?") },
        text = { Text("You'll lose the sets you've logged this session.") },
        confirmButton = {
            Button(onClick = onConfirm) { Text("Discard") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Keep going") }
        }
    )
}

@Composable
private fun RestTimerSetterDialog(
    exerciseName: String,
    currentSeconds: Int?,
    onSet: (Int) -> Unit,
    onClear: () -> Unit,
    onDismiss: () -> Unit
) {
    val presets = listOf(60 to "1m", 90 to "1:30", 120 to "2m", 150 to "2:30", 180 to "3m", 240 to "4m")
    var selected by remember { mutableStateOf(currentSeconds) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Rest timer — $exerciseName") },
        text = {
            @OptIn(ExperimentalLayoutApi::class)
        FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                presets.forEach { (secs, label) ->
                    FilterChip(
                        selected = selected == secs,
                        onClick = { selected = secs },
                        label = { Text(label) }
                    )
                }
            }
        },
        confirmButton = {
            Button(onClick = { selected?.let { onSet(it) } }, enabled = selected != null) { Text("Save") }
        },
        dismissButton = {
            if (currentSeconds != null) {
                TextButton(onClick = onClear) { Text("Use default") }
            } else {
                TextButton(onClick = onDismiss) { Text("Cancel") }
            }
        }
    )
}

@Composable
private fun GoalSetterDialog(
    exerciseName: String,
    currentGoal: Double?,
    onSet: (Double) -> Unit,
    onClear: () -> Unit,
    onDismiss: () -> Unit
) {
    var weightText by remember { mutableStateOf(currentGoal?.toInt()?.toString() ?: "") }
    val weightLb = weightText.toDoubleOrNull()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Goal weight — $exerciseName") },
        text = {
            OutlinedTextField(
                value = weightText,
                onValueChange = { weightText = it.filter { c -> c.isDigit() || c == '.' } },
                label = { Text("Target weight (lb)") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal)
            )
        },
        confirmButton = {
            Button(
                onClick = { weightLb?.let { onSet(it) } },
                enabled = weightLb != null && weightLb > 0
            ) { Text("Save") }
        },
        dismissButton = {
            if (currentGoal != null) {
                TextButton(onClick = onClear) { Text("Clear goal") }
            } else {
                TextButton(onClick = onDismiss) { Text("Cancel") }
            }
        }
    )
}

@Composable
private fun SessionHero(
    state: DayUiState,
    onBack: () -> Unit,
    onFinish: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline

    val sessionDateText = remember(state.sessionStartedAt) {
        val ms = state.sessionStartedAt ?: System.currentTimeMillis()
        SimpleDateFormat("EEE · MMM d · HH:mm", Locale.getDefault())
            .format(Date(ms)).uppercase()
    }

    val estimatedEndText = remember(state.remainingSetsCount) {
        val remaining = state.remainingSetsCount
        if (remaining <= 0) null
        else {
            val endMs = System.currentTimeMillis() + remaining * 3L * 60_000
            SimpleDateFormat("h:mm a", Locale.getDefault()).format(Date(endMs))
        }
    }

    val exerciseCount = state.exercises.size
    val doneCount = state.exercises.count { it.loggedSets.size >= it.plan.sets || it.skipped }
    val totalSets = state.exercises.sumOf { it.loggedSets.size }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
    ) {
        // Nav row: [back arrow + session name italic] | [finish]
        Row(
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onBack) {
                    Icon(
                        Icons.AutoMirrored.Filled.ArrowBack,
                        contentDescription = "Back",
                        tint = onBg
                    )
                }
                Text(
                    state.displayName,
                    style = MaterialTheme.typography.headlineMedium,
                    color = onBg,
                    fontStyle = FontStyle.Italic
                )
            }
            Text(
                "finish",
                style = MaterialTheme.typography.labelMedium,
                color = muted,
                modifier = Modifier
                    .clickable { onFinish() }
                    .padding(start = 8.dp, top = 8.dp, bottom = 8.dp)
            )
        }

        // Subtitle: WORD · DONE ~ estimated end
        val subtitleText = if (estimatedEndText != null)
            "${state.dayPlan.word} · DONE ~ $estimatedEndText"
        else state.dayPlan.word
        Text(
            subtitleText,
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontSize = 11.sp
        )

        Spacer(Modifier.height(16.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.3f))
        Spacer(Modifier.height(12.dp))

        // Date from session start
        Text(sessionDateText, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
        Spacer(Modifier.height(6.dp))

        // "Six moves today." with italic "today."
        val countWord = when (exerciseCount) {
            1 -> "One"; 2 -> "Two"; 3 -> "Three"; 4 -> "Four"; 5 -> "Five"
            6 -> "Six"; 7 -> "Seven"; 8 -> "Eight"; 9 -> "Nine"; 10 -> "Ten"
            11 -> "Eleven"; 12 -> "Twelve"; else -> "$exerciseCount"
        }
        val movesText = buildAnnotatedString {
            append("$countWord moves ")
            withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { append("today.") }
        }
        Text(movesText, style = MaterialTheme.typography.displayLarge, color = onBg)

        Spacer(Modifier.height(6.dp))

        // Progress line: "0 of 6 done · 3 sets logged."
        val progressLine = buildString {
            append("$doneCount of $exerciseCount done")
            if (totalSets > 0) append(" · $totalSets sets logged.") else append(".")
        }
        Text(
            progressLine,
            style = MaterialTheme.typography.bodySmall,
            color = onBg.copy(alpha = 0.45f),
            fontStyle = FontStyle.Italic
        )

        Spacer(Modifier.height(12.dp))

        // Segmented progress bar — one segment per exercise
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(3.dp)
        ) {
            state.exercises.forEach { ex ->
                val isDone = ex.loggedSets.size >= ex.plan.sets || ex.skipped
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .height(2.dp)
                        .background(
                            if (isDone) onBg else outline.copy(alpha = 0.25f),
                            RoundedCornerShape(1.dp)
                        )
                )
            }
        }

        Spacer(Modifier.height(16.dp))
    }
}

