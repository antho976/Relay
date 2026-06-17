package com.forge.app.ui.gym.train

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
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
import androidx.compose.foundation.layout.width
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.units.toStoredWeightText
import com.forge.app.program.ExerciseUnit
import com.forge.app.ui.common.FirstTouchTip
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.gym.train.components.CollapsedRow
import com.forge.app.ui.gym.train.components.ExerciseCard
import com.forge.app.ui.gym.train.components.ExerciseChartSheet
import com.forge.app.ui.gym.train.components.UpNextBubble
import com.forge.app.ui.gym.train.components.WarmupGate
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.DayUiState
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.ForgeMotion
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
    var chartForExerciseId by remember { mutableStateOf<String?>(null) }

    chartForExerciseId?.let { exId ->
        val exercise = state.exercises.firstOrNull { it.plan.id == exId }
        if (exercise != null) {
            ExerciseChartSheet(
                exerciseName = exercise.effectiveName,
                history = exercise.sessionHistory,
                onDismiss = { chartForExerciseId = null }
            )
        }
    }

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
    val firstWorkoutDone = LocalForgeSettings.current.firstWorkoutDone

    // Single-exercise focus. The shown exercise is an explicit selection — it does NOT
    // auto-advance when sets complete. The user advances manually via the "MOVE TO NEXT"
    // CTA (shown once target sets are met) or by tapping an exercise in the UP NEXT bubble.
    // Keyed on the exercise list (same reference across rest-timer-tick state copies), so this list
    // walk doesn't re-run on every per-second recomposition — only when the exercises actually change.
    val firstIncompleteId = remember(state.exercises) {
        state.exercises.firstOrNull { !it.skipped && it.loggedSets.size < it.targetSets }?.plan?.id
    }
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

            // First-touch (D8/D10): a brand-new user opens to a list of names with no obvious action.
            // Gated on the lifetime "first workout done" flag (not per-exercise history) so it shows only
            // for a genuinely new user — never for a returning user starting an all-new program. Auto-hides
            // after the first set of this session.
            if (!firstWorkoutDone && state.exercises.isNotEmpty() && state.exercises.all { it.loggedSets.isEmpty() }) {
                item(key = "first-time-tip") {
                    FirstTouchTip(
                        "Your week starts here.",
                        "Tap an exercise to open it, then log each set with the weight and reps you hit. The coach starts learning from your very first session.",
                        Modifier.padding(horizontal = 24.dp, vertical = 8.dp)
                    )
                }
            }

            // ── Suggested order (engine System 3) — pre-work only ────────────
            val ordering = state.orderingSuggestion
            if (ordering != null && state.exercises.all { it.loggedSets.isEmpty() }) {
                item(key = "ordering-suggestion") {
                    val muted = MaterialTheme.colorScheme.onSurfaceVariant
                    val accent = MaterialTheme.colorScheme.primary
                    val nameById = state.exercises.associate { it.plan.id to it.effectiveName }
                    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 10.dp)) {
                        Text("SUGGESTED ORDER", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                        Spacer(Modifier.height(4.dp))
                        Text(
                            ordering.orderedExerciseIds.mapNotNull { nameById[it] }.joinToString("  →  "),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onBackground
                        )
                        Spacer(Modifier.height(2.dp))
                        Text(ordering.reason, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.8f))
                        Spacer(Modifier.height(6.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                            Text(
                                "Apply →",
                                style = MaterialTheme.typography.labelSmall, color = accent,
                                modifier = Modifier.clickableLabeled("Apply the suggested order") { onEvent(DayUiEvent.ApplyOrderingSuggestion) }.padding(vertical = 2.dp)
                            )
                            Text(
                                "dismiss",
                                style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f),
                                modifier = Modifier.clickableLabeled("Dismiss the suggestion") { onEvent(DayUiEvent.DismissOrderingSuggestion) }.padding(vertical = 2.dp)
                            )
                        }
                    }
                    HorizontalDivider(
                        modifier = Modifier.padding(horizontal = 24.dp),
                        color = MaterialTheme.colorScheme.outline.copy(alpha = 0.2f)
                    )
                }
            }

            if (shownExercise != null) {
                val idx = state.exercises.indexOf(shownExercise)
                // "Up next" = every OTHER exercise still needing work (not the shown one, not
                // skipped, not yet at target). Ordered next-after-current first, then any that sit
                // earlier in the list — so jumping back to a finished exercise can't orphan an
                // incomplete one (it lands here, not nowhere). Completed/skipped ones go to DONE.
                val remaining = state.exercises.withIndex().filter { (_, ex) ->
                    ex.plan.id != shownExercise.plan.id && !ex.skipped && ex.loggedSets.size < ex.targetSets
                }
                val upcoming = (remaining.filter { it.index > idx } + remaining.filter { it.index < idx })
                    .map { it.index to it.value }
                val nextEx = upcoming.firstOrNull()?.second

                item(key = "current-exercise") {
                    // Shared-axis X keyed on the exercise id (not the whole state) so it only
                    // animates on an actual exercise switch — not on every set logged. The new
                    // exercise slides in from the direction of travel (right = later in the list).
                    AnimatedContent(
                        targetState = shownExercise.plan.id,
                        transitionSpec = {
                            val fromIdx = state.exercises.indexOfFirst { it.plan.id == initialState }
                            val toIdx = state.exercises.indexOfFirst { it.plan.id == targetState }
                            val dir = if (toIdx >= fromIdx) 1 else -1
                            (slideInHorizontally(ForgeMotion.enterTween()) { (it / 4) * dir } +
                                fadeIn(ForgeMotion.enterTween())) togetherWith
                                (slideOutHorizontally(ForgeMotion.exitTween()) { -(it / 4) * dir } +
                                    fadeOut(ForgeMotion.exitTween()))
                        },
                        label = "exercise-switch"
                    ) { id ->
                        val ex = state.exercises.firstOrNull { it.plan.id == id }
                        if (ex != null) {
                            val exIdx = state.exercises.indexOfFirst { it.plan.id == id }
                            val exIsNow = id == firstIncompleteId
                            val exNextId = state.exercises
                                .withIndex()
                                .firstOrNull { it.index > exIdx && !it.value.skipped }
                                ?.value?.plan?.id
                            ExerciseCard(
                                exerciseIndex = exIdx,
                                // Force-expand: the focused view always shows the full ledger.
                                state = ex.copy(isExpanded = true),
                                isNow = exIsNow,
                                totalExercises = state.exercises.size,
                                // The rest timer renders inside the card (right under the set log),
                                // not far below it — passed off state.restTimer so it appears the
                                // instant a set is logged (plan.id is unchanged → no re-animation).
                                restTimerState = state.restTimer,
                                sessionStartedAtMs = state.elapsedAnchorMs,
                                advanceLabel = if (exNextId != null) "MOVE TO NEXT →" else "FINISH WORKOUT →",
                                onAdvance = { if (exNextId != null) shownExerciseId = exNextId else onEvent(DayUiEvent.FinishWorkout) },
                                onToggle = { },
                                onLogSet = { weight, reps ->
                                    // Plate exercises enter a plate COUNT (not a display-unit weight),
                                    // so skip the kg→lb conversion — WeightParser turns the count into lb.
                                    val stored = if (ex.plan.unit == ExerciseUnit.PLATES) weight else toStoredWeightText(weight, useKg)
                                    onEvent(DayUiEvent.LogSet(id, stored, reps))
                                },
                                onDeleteSet = { setId -> onEvent(DayUiEvent.DeleteSet(setId)) },
                                // Same unit handling as logging — plate counts pass through, free weights convert.
                                onEditSet = { setId, w, r ->
                                    val stored = if (ex.plan.unit == ExerciseUnit.PLATES) w else toStoredWeightText(w, useKg)
                                    onEvent(DayUiEvent.EditSet(setId, stored, r))
                                },
                                onLogSameAsLast = { setId -> onEvent(DayUiEvent.LogSameAsLast(id, setId)) },
                                onRate = { rating -> onEvent(DayUiEvent.RateExercise(id, rating)) },
                                onNoteChange = { note -> onEvent(DayUiEvent.UpdateNote(id, note)) },
                                onToggleSkipped = {
                                    onEvent(DayUiEvent.ToggleSkipped(id))
                                    // Skipping the focused exercise jumps to the next one — the skipped
                                    // exercise drops into the DONE / SKIPPED section below. Un-skipping
                                    // leaves you on it.
                                    if (!ex.skipped) {
                                        val advanceTo = exNextId
                                            ?: state.exercises.firstOrNull {
                                                it.plan.id != id && !it.skipped && it.loggedSets.size < it.targetSets
                                            }?.plan?.id
                                        if (advanceTo != null) shownExerciseId = advanceTo
                                    }
                                },
                                onOpenSwapPicker = { onEvent(DayUiEvent.OpenSwapPicker(id)) },
                                onOpenGoalSetter = { onEvent(DayUiEvent.OpenGoalSetter(id)) },
                                onLongPress = { onEvent(DayUiEvent.LongPressExercise(id)) },
                                onOpenRestTimerSetter = { restTimerSetterForId = id },
                                onSkipRest = { onEvent(DayUiEvent.RestTimerSkip) },
                                onSetExerciseUnit = { unit -> onEvent(DayUiEvent.SetExerciseUnit(id, unit)) },
                                onPinNote = { note -> onEvent(DayUiEvent.SetPinnedNote(id, note)) },
                                onToggleSetDifficultyTag = { setId, tag -> onEvent(DayUiEvent.ToggleSetDifficultyTag(setId, tag)) },
                                onSetRpe = { setId, rpe -> onEvent(DayUiEvent.SetRpe(setId, rpe)) },
                                onAddSet = { onEvent(DayUiEvent.AddBonusSet(id)) },
                                onOpenChart = { chartForExerciseId = id }
                            )
                        }
                    }
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

                // ── DONE ─────────────────────────────────────────────────────────
                // Exercises you've finished or skipped stay here, below Up Next — tap one to
                // re-open it in the card above and keep editing its sets.
                val done = state.exercises.withIndex().filter { (_, ex) ->
                    ex.plan.id != shownExercise.plan.id && (ex.skipped || ex.loggedSets.size >= ex.targetSets)
                }
                if (done.isNotEmpty()) {
                    item(key = "done-header") {
                        val outline = MaterialTheme.colorScheme.outline
                        HorizontalDivider(color = outline.copy(alpha = 0.2f))
                        Text(
                            "DONE / SKIPPED",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            fontSize = 9.sp,
                            modifier = Modifier.padding(start = 24.dp, top = 14.dp, bottom = 2.dp)
                        )
                    }
                    // Key by position, not exercise id — a day can legitimately hold the same exercise
                    // twice (a tiny equipment pool), and duplicate keys crash LazyColumn.
                    items(done, key = { "done-${it.index}" }) { entry ->
                        val ex = entry.value
                        CollapsedRow(
                            exerciseIndex = entry.index,
                            state = ex,
                            isNow = false,
                            onToggle = { shownExerciseId = ex.plan.id },
                            onOpenSwapPicker = { onEvent(DayUiEvent.OpenSwapPicker(ex.plan.id)) }
                        )
                        HorizontalDivider(
                            modifier = Modifier.padding(horizontal = 24.dp),
                            color = MaterialTheme.colorScheme.outline.copy(alpha = 0.12f)
                        )
                    }
                    item(key = "done-bottom") { Spacer(Modifier.height(16.dp)) }
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

    Column(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            // Left: back + day name. weight(1f) so it yields space to the date pill + FINISH.
            Row(
                modifier = Modifier.weight(1f),
                verticalAlignment = Alignment.CenterVertically
            ) {
                IconButton(onClick = onBack, modifier = Modifier.padding(end = 2.dp)) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = onBg)
                }
                Text(
                    state.displayName.uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    color = onBg,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
            Spacer(Modifier.width(8.dp))
            Box(modifier = Modifier.border(1.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(50)).padding(horizontal = 10.dp, vertical = 4.dp)) {
                Text(datePillText, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, maxLines = 1)
            }
            Spacer(Modifier.width(8.dp))
            Box(modifier = Modifier.background(Color.White, RoundedCornerShape(50)).clickableLabeled("Finish session") { onFinish() }.padding(horizontal = 16.dp, vertical = 7.dp)) {
                Text("FINISH", style = MaterialTheme.typography.labelSmall, color = Color.Black, maxLines = 1)
            }
        }
        // "Beat the ghost" running scoreboard — the duel vs last session, live as you log.
        if (state.ghostComparable > 0) {
            val beats = state.ghostBeats
            val total = state.ghostComparable
            val winning = beats * 2 >= total
            Row(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                Text(
                    if (winning) "⚡ BEATING LAST SESSION" else "VS LAST SESSION",
                    style = MaterialTheme.typography.labelSmall,
                    color = if (winning) ForgeLastGreen else muted,
                    fontSize = 9.sp
                )
                Text(
                    "$beats / $total sets",
                    style = MaterialTheme.typography.labelSmall,
                    color = if (winning) ForgeLastGreen else muted,
                    fontSize = 9.sp
                )
            }
        }
        HorizontalDivider(color = outline.copy(alpha = 0.2f))
    }
}
