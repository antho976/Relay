package com.forge.app.ui.gym.train

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Done
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.gym.train.components.ExerciseCard
import com.forge.app.ui.gym.train.components.RestTimerBubble
import com.forge.app.ui.gym.train.components.RestTimerControlsDialog
import com.forge.app.ui.gym.train.components.SessionSummarySheet
import com.forge.app.ui.gym.train.components.SwapPickerSheet
import com.forge.app.ui.gym.train.components.WarmupGate
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.DayUiState

@Suppress("UNUSED_PARAMETER") // dayKey arrives via SavedStateHandle; not used here
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DayScreen(
    dayKey: String,
    onBack: () -> Unit,
    viewModel: DayViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

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
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(state.displayName, style = MaterialTheme.typography.titleLarge)
                        Text(
                            state.dayPlan.word,
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.primary
                        )
                    }
                },
                navigationIcon = {
                    IconButton(onClick = { viewModel.onEvent(DayUiEvent.RequestBack) }) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    IconButton(onClick = { viewModel.onEvent(DayUiEvent.FinishWorkout) }) {
                        Icon(
                            Icons.Default.Done,
                            contentDescription = "Finish workout",
                            tint = MaterialTheme.colorScheme.primary
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background
                )
            )
        },
        floatingActionButton = {
            state.restTimer?.let { timer ->
                RestTimerBubble(
                    state = timer,
                    onOpenControls = { viewModel.onEvent(DayUiEvent.RestTimerOpen) }
                )
            }
        },
        containerColor = MaterialTheme.colorScheme.background
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
            onDismiss = { viewModel.onEvent(DayUiEvent.RestTimerClose) }
        )
    }

    state.swapPickerExercise?.let { exerciseUi ->
        SwapPickerSheet(
            forExercise = exerciseUi.plan,
            hasPersistentSwap = exerciseUi.persistentSwapName != null,
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

    state.summary?.let { summary ->
        SessionSummarySheet(
            summary = summary,
            onDismiss = { mood -> viewModel.onEvent(DayUiEvent.DismissSummary(mood)) }
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

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        if (!state.isWarmupComplete) {
            item(key = "warmup") {
                WarmupGate(
                    warmupItems = state.dayPlan.warmup,
                    checks = state.warmupChecks,
                    onToggle = { idx -> onEvent(DayUiEvent.ToggleWarmupItem(idx)) },
                    onSkip = { onEvent(DayUiEvent.SkipWarmup) }
                )
            }
        } else {
            items(state.exercises, key = { it.plan.id }) { exerciseState ->
                ExerciseCard(
                    state = exerciseState,
                    onToggle = { onEvent(DayUiEvent.ToggleExpanded(exerciseState.plan.id)) },
                    onLogSet = { weight, reps ->
                        onEvent(DayUiEvent.LogSet(exerciseState.plan.id, weight, reps))
                    },
                    onDeleteSet = { setId -> onEvent(DayUiEvent.DeleteSet(setId)) },
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
                    onOpenSwapPicker = { onEvent(DayUiEvent.OpenSwapPicker(exerciseState.plan.id)) }
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
