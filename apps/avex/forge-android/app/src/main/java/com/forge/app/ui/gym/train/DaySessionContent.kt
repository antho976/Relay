package com.forge.app.ui.gym.train

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.units.parseToLb
import com.forge.app.ui.gym.train.components.ExerciseCard
import com.forge.app.ui.gym.train.components.UpNextBubble
import com.forge.app.ui.gym.train.components.WarmupGate
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.DayUiState
import com.forge.app.ui.theme.LocalForgeSettings
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
internal fun DayContent(state: DayUiState, onEvent: (DayUiEvent) -> Unit) {
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

    // Single-exercise focus. The shown exercise is an explicit selection — it does NOT
    // auto-advance when sets complete. The user advances manually via the "MOVE TO NEXT"
    // CTA (shown once target sets are met) or by tapping an exercise in the UP NEXT bubble.
    val firstIncompleteId = state.exercises.firstOrNull { !it.skipped && it.loggedSets.size < it.targetSets }?.plan?.id
    var shownExerciseId by remember { mutableStateOf<String?>(null) }
    // Initialise the selection (and repair it if the shown exercise disappears), without
    // re-pointing it as exercises get completed.
    LaunchedEffect(state.exercises.map { it.plan.id }) {
        if (shownExerciseId == null || state.exercises.none { it.plan.id == shownExerciseId }) {
            shownExerciseId = firstIncompleteId ?: state.exercises.firstOrNull()?.plan?.id
        }
    }
    val shownExercise = state.exercises.firstOrNull { it.plan.id == shownExerciseId }
        ?: state.exercises.firstOrNull()

    LazyColumn(modifier = Modifier.fillMaxSize(), contentPadding = PaddingValues(vertical = 8.dp)) {
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
                SessionHero(state = state, onBack = { onEvent(DayUiEvent.RequestBack) }, onFinish = { onEvent(DayUiEvent.FinishWorkout) })
            }

            if (shownExercise != null) {
                val shownId = shownExercise.plan.id
                val idx = state.exercises.indexOf(shownExercise)
                val isNowExercise = shownId == firstIncompleteId
                val upcoming = state.exercises
                    .withIndex()
                    .filter { it.index > idx && !it.value.skipped }
                    .map { it.index to it.value }
                val nextEx = upcoming.firstOrNull()?.second
                val nextId = nextEx?.plan?.id
                val advanceLabel = if (nextId != null) "MOVE TO NEXT →" else "FINISH WORKOUT →"
                val onAdvance: () -> Unit = {
                    if (nextId != null) shownExerciseId = nextId else onEvent(DayUiEvent.FinishWorkout)
                }

                item(key = "current-exercise") {
                    ExerciseCard(
                        exerciseIndex = idx,
                        // Force-expand: the focused view always shows the full ledger.
                        state = shownExercise.copy(isExpanded = true),
                        isNow = isNowExercise,
                        totalExercises = state.exercises.size,
                        restTimerState = if (isNowExercise) state.restTimer else null,
                        advanceLabel = advanceLabel,
                        onAdvance = onAdvance,
                        onToggle = { },
                        onLogSet = { weight, reps ->
                            val storedWeight = if (useKg) {
                                val lb = parseToLb(weight, useKg = true)
                                if (lb != null) "%.1f".format(lb).trimEnd('0').trimEnd('.') else weight
                            } else weight
                            onEvent(DayUiEvent.LogSet(shownId, storedWeight, reps))
                        },
                        onDeleteSet = { setId -> onEvent(DayUiEvent.DeleteSet(setId)) },
                        onEditSet = { setId, w, r -> onEvent(DayUiEvent.EditSet(setId, w, r)) },
                        onLogSameAsLast = { setId -> onEvent(DayUiEvent.LogSameAsLast(shownId, setId)) },
                        onRate = { rating -> onEvent(DayUiEvent.RateExercise(shownId, rating)) },
                        onNoteChange = { note -> onEvent(DayUiEvent.UpdateNote(shownId, note)) },
                        onToggleSkipped = { onEvent(DayUiEvent.ToggleSkipped(shownId)) },
                        onOpenSwapPicker = { onEvent(DayUiEvent.OpenSwapPicker(shownId)) },
                        onOpenGoalSetter = { onEvent(DayUiEvent.OpenGoalSetter(shownId)) },
                        onLongPress = { onEvent(DayUiEvent.LongPressExercise(shownId)) },
                        onOpenRestTimerSetter = { restTimerSetterForId = shownId },
                        onSetExerciseUnit = { unit -> onEvent(DayUiEvent.SetExerciseUnit(shownId, unit)) },
                        onPinNote = { note -> onEvent(DayUiEvent.SetPinnedNote(shownId, note)) },
                        onToggleSetDifficultyTag = { setId, tag -> onEvent(DayUiEvent.ToggleSetDifficultyTag(setId, tag)) },
                        onSetRpe = { setId, rpe -> onEvent(DayUiEvent.SetRpe(setId, rpe)) },
                        onAddSet = { onEvent(DayUiEvent.AddBonusSet(shownId)) }
                    )
                }

                item(key = "up-next") {
                    Spacer(Modifier.height(14.dp))
                    UpNextBubble(
                        nextName = nextEx?.effectiveName,
                        nextTarget = nextEx?.let { "${it.plan.sets} × ${it.plan.reps}" },
                        nextDelta = shownExercise.nextSuggestedWeightDelta,
                        upcoming = upcoming,
                        onSelectExercise = { id -> shownExerciseId = id },
                        onOpenSwapPicker = { id -> onEvent(DayUiEvent.OpenSwapPicker(id)) },
                        onAddExercise = { onEvent(DayUiEvent.OpenAddExercisePicker) }
                    )
                    Spacer(Modifier.height(16.dp))
                }
            }
        }
    }
}

@Composable
internal fun SessionHero(state: DayUiState, onBack: () -> Unit, onFinish: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline

    val datePillText = remember(state.sessionStartedAt) {
        val ms = state.sessionStartedAt ?: System.currentTimeMillis()
        SimpleDateFormat("EEE · MMM d", Locale.getDefault()).format(Date(ms)).uppercase()
    }

    val estimatedEndText = remember(state.remainingSetsCount) {
        val remaining = state.remainingSetsCount
        if (remaining <= 0) null
        else {
            val endMs = System.currentTimeMillis() + remaining * 3L * 60_000
            SimpleDateFormat("h:mm a", Locale.getDefault()).format(Date(endMs))
        }
    }

    Column(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onBack, modifier = Modifier.padding(end = 2.dp)) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = onBg)
                }
                Column {
                    Text(state.displayName.uppercase(), style = MaterialTheme.typography.labelSmall, color = onBg)
                    val subtitle = buildString {
                        append(state.dayPlan.word)
                        estimatedEndText?.let { append(" · DONE ~ $it") }
                    }
                    if (subtitle.isNotBlank()) {
                        Text(subtitle, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                    }
                }
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(modifier = Modifier.border(1.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(50)).padding(horizontal = 10.dp, vertical = 4.dp)) {
                    Text(datePillText, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                }
                Box(modifier = Modifier.background(Color.White, RoundedCornerShape(50)).clickable { onFinish() }.padding(horizontal = 16.dp, vertical = 7.dp)) {
                    Text("FINISH", style = MaterialTheme.typography.labelSmall, color = Color.Black)
                }
            }
        }
        HorizontalDivider(color = outline.copy(alpha = 0.2f))
    }
}
