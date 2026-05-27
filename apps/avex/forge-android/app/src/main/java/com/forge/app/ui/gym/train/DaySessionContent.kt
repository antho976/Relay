package com.forge.app.ui.gym.train

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.units.parseToLb
import com.forge.app.ui.gym.train.components.ExerciseCard
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
            val nowExerciseId = state.exercises.firstOrNull { !it.skipped && it.loggedSets.size < it.plan.sets }?.plan?.id
            val nowIdx = state.exercises.indexOfFirst { it.plan.id == nowExerciseId }
            val nextExerciseName = if (nowIdx >= 0) state.exercises.drop(nowIdx + 1).firstOrNull { !it.skipped }?.effectiveName else null
            items(state.exercises, key = { it.plan.id }) { exerciseState ->
                val idx = state.exercises.indexOf(exerciseState)
                val isNowExercise = exerciseState.plan.id == nowExerciseId
                ExerciseCard(
                    exerciseIndex = idx,
                    state = exerciseState,
                    isNow = isNowExercise,
                    totalExercises = state.exercises.size,
                    restTimerState = if (isNowExercise) state.restTimer else null,
                    nextExerciseName = if (isNowExercise) nextExerciseName else null,
                    onToggle = { onEvent(DayUiEvent.ToggleExpanded(exerciseState.plan.id)) },
                    onLogSet = { weight, reps ->
                        val storedWeight = if (useKg) {
                            val lb = parseToLb(weight, useKg = true)
                            if (lb != null) "%.1f".format(lb).trimEnd('0').trimEnd('.') else weight
                        } else weight
                        onEvent(DayUiEvent.LogSet(exerciseState.plan.id, storedWeight, reps))
                    },
                    onDeleteSet = { setId -> onEvent(DayUiEvent.DeleteSet(setId)) },
                    onEditSet = { setId, w, r -> onEvent(DayUiEvent.EditSet(setId, w, r)) },
                    onLogSameAsLast = { setId -> onEvent(DayUiEvent.LogSameAsLast(exerciseState.plan.id, setId)) },
                    onRate = { rating -> onEvent(DayUiEvent.RateExercise(exerciseState.plan.id, rating)) },
                    onNoteChange = { note -> onEvent(DayUiEvent.UpdateNote(exerciseState.plan.id, note)) },
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
                    modifier = Modifier.fillMaxWidth().clickable { onEvent(DayUiEvent.OpenAddExercisePicker) }
                        .padding(horizontal = 24.dp, vertical = 16.dp)
                )
            }
        }
    }
}

@Composable
internal fun SessionHero(state: DayUiState, onBack: () -> Unit, onFinish: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val bg = MaterialTheme.colorScheme.background

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
                Box(modifier = Modifier.background(onBg, RoundedCornerShape(50)).clickable { onFinish() }.padding(horizontal = 14.dp, vertical = 6.dp)) {
                    Text("FINISH", style = MaterialTheme.typography.labelSmall, color = bg)
                }
            }
        }
        HorizontalDivider(color = outline.copy(alpha = 0.2f))
    }
}
