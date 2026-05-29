package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.timer.RestTimerState
import com.forge.app.ui.gym.train.state.ExerciseUiState

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ExerciseCard(
    exerciseIndex: Int,
    state: ExerciseUiState,
    isNow: Boolean = false,
    totalExercises: Int = 0,
    restTimerState: RestTimerState? = null,
    onToggle: () -> Unit,
    onLogSet: (weightText: String, reps: Int) -> Unit,
    onDeleteSet: (setId: Long) -> Unit,
    onEditSet: (setId: Long, weightText: String, reps: Int) -> Unit,
    onLogSameAsLast: (setId: Long) -> Unit,
    onRate: (EffortRating) -> Unit,
    onNoteChange: (String) -> Unit,
    onToggleSkipped: () -> Unit,
    onOpenSwapPicker: () -> Unit,
    onOpenGoalSetter: () -> Unit = {},
    onOpenRestTimerSetter: () -> Unit = {},
    onSetExerciseUnit: (String?) -> Unit = {},
    onPinNote: (String) -> Unit = {},
    onToggleSetDifficultyTag: (setId: Long, currentTag: String?) -> Unit = { _, _ -> },
    onSetRpe: (setId: Long, rpe: Double?) -> Unit = { _, _ -> },
    onAddSet: () -> Unit = {},
    onSwitchUnit: () -> Unit = {},
    onOpenChart: () -> Unit = {},
    advanceLabel: String = "",
    onAdvance: () -> Unit = {},
    onMoveUp: (() -> Unit)? = null,
    onMoveDown: (() -> Unit)? = null,
    onLongPress: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline

    val cardAlpha = if (state.skipped) 0.45f else 1f
    val longPressModifier = if (onLongPress != null)
        Modifier.combinedClickable(onClick = {}, onLongClick = onLongPress)
    else Modifier

    Column(
        modifier = modifier
            .fillMaxWidth()
            .alpha(cardAlpha)
            .then(longPressModifier)
    ) {
        if (!state.isExpanded) {
            // ── Collapsed row ──────────────────────────────────────────────────
            CollapsedRow(
                exerciseIndex = exerciseIndex,
                state = state,
                isNow = isNow,
                onToggle = onToggle,
                onOpenSwapPicker = onOpenSwapPicker
            )
        } else {
            // ── Expanded ledger card ────────────────────────────────────────────
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 24.dp)
            ) {
                Spacer(Modifier.height(8.dp))

                // Exercise counter
                if (totalExercises > 0) {
                    Text(
                        "EXERCISE ${"%02d".format(exerciseIndex + 1)} / ${"%02d".format(totalExercises)}",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp
                    )
                    Spacer(Modifier.height(4.dp))
                }

                // Exercise name (large serif) — tap to open the swap picker.
                Text(
                    state.effectiveName,
                    style = MaterialTheme.typography.displayLarge,
                    color = onBg,
                    textDecoration = if (state.skipped) TextDecoration.LineThrough else TextDecoration.None,
                    modifier = Modifier.clickable { onOpenSwapPicker() }
                )

                Spacer(Modifier.height(6.dp))

                // Target + last session line
                val priorLastSet = state.priorSets.lastOrNull()
                val targetText = buildAnnotatedString {
                    withStyle(SpanStyle(fontStyle = FontStyle.Italic)) {
                        append("Target ${state.plan.sets} × ${state.plan.reps}")
                        if (priorLastSet != null) {
                            append(" · last session ${priorLastSet.weightText} × ${priorLastSet.reps}")
                            priorLastSet.rpe?.let { rpe ->
                                val rpeStr = if (rpe % 1.0 == 0.0) "${rpe.toInt()}" else "%.1f".format(rpe)
                                append(" @ RPE $rpeStr")
                            }
                        }
                    }
                }
                Text(targetText, style = MaterialTheme.typography.bodySmall, color = muted)

                // Suggested next line
                if (state.suggestedWeight != null) {
                    Spacer(Modifier.height(2.dp))
                    val suggLine = buildAnnotatedString {
                        withStyle(SpanStyle(fontStyle = FontStyle.Italic)) {
                            append("Suggested next → ${state.suggestedWeight}")
                            if (!state.suggestionReason.isNullOrBlank()) {
                                append(" (${state.suggestionReason})")
                            }
                        }
                    }
                    Text(suggLine, style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.75f))
                }

                // Pinned cue
                if (state.pinnedNote.isNotBlank()) {
                    Spacer(Modifier.height(4.dp))
                    Text(
                        "\" ${state.pinnedNote} \"",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted.copy(alpha = 0.6f),
                        fontStyle = FontStyle.Italic
                    )
                }

                // Last-session strip + comparison sparkline (current vs previous).
                if (state.sessionHistory.isNotEmpty()) {
                    Spacer(Modifier.height(12.dp))
                    LastSessionStrip(
                        history = state.sessionHistory,
                        currentWeights = state.loggedSets.mapNotNull { it.weightLb },
                        priorWeights = state.priorSets.mapNotNull { it.weightLb },
                        onClick = onOpenChart
                    )
                }

                Spacer(Modifier.height(16.dp))

                // Session stats chip (sets progress + volume)
                if (state.loggedSets.isNotEmpty()) {
                    val totalVolumeLb = state.loggedSets.sumOf { (it.weightLb ?: 0.0) * it.reps }
                    val volumeText = if (totalVolumeLb > 0) "  ·  ${totalVolumeLb.toInt()} LB" else ""
                    Text(
                        "${state.loggedSets.size} / ${state.targetSets} SETS$volumeText",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp
                    )
                    Spacer(Modifier.height(8.dp))
                }

                // ── Set table ─────────────────────────────────────────────────
                // Table header (5 cols: SET | WEIGHT | REPS | RPE | Δ LAST)
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("SET", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.width(36.dp))
                    Text("WEIGHT · LB", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.weight(1f))
                    Text("REPS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.width(48.dp))
                    Text("RPE", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.width(44.dp), textAlign = TextAlign.Center)
                    Text("△ LAST", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.width(72.dp), textAlign = TextAlign.End)
                }
                HorizontalDivider(color = outline.copy(alpha = 0.3f), modifier = Modifier.padding(top = 4.dp))

                // Logged set rows, with the actual rest taken shown between consecutive sets
                state.loggedSets.forEachIndexed { i, set ->
                    SetRow(
                        set = set,
                        setIndex = i + 1,
                        isPr = set.id in state.prSetIds,
                        priorSet = state.priorSets.getOrNull(i),
                        onDelete = { onDeleteSet(set.id) },
                        onEdit = { w, r -> onEditSet(set.id, w, r) },
                        onLongPress = { onLogSameAsLast(set.id) },
                        onToggleDifficultyTag = { tag -> onToggleSetDifficultyTag(set.id, tag) },
                        onSetRpe = { rpe -> onSetRpe(set.id, rpe) }
                    )
                    val next = state.loggedSets.getOrNull(i + 1)
                    if (next != null) {
                        val restSec = ((next.completedAt - set.completedAt) / 1000L).toInt()
                        RestBetweenSets(restSec)
                    }
                    HorizontalDivider(color = outline.copy(alpha = 0.12f))
                }

                // Inline rest timer row
                restTimerState?.let { timer ->
                    Spacer(Modifier.height(8.dp))
                    InlineRestTimer(timer = timer, onTap = onOpenRestTimerSetter)
                    Spacer(Modifier.height(8.dp))
                }

                // Input row for the next set
                if (!state.skipped) {
                    val targetsMet = state.loggedSets.size >= state.targetSets
                    Spacer(Modifier.height(if (restTimerState == null) 8.dp else 0.dp))
                    SetInputRow(
                        prefillWeight = state.prefillWeight,
                        suggestedWeight = state.suggestedWeight,
                        suggestionReason = state.suggestionReason,
                        priorSets = state.priorSets,
                        nextSetNumber = state.loggedSets.size + 1,
                        priorSetForActiveRow = state.priorSets.getOrNull(state.loggedSets.size),
                        targetsMet = targetsMet,
                        advanceLabel = advanceLabel,
                        onAdvance = onAdvance,
                        onSubmit = onLogSet,
                        onAddSet = onAddSet,
                        onSwitchUnit = onSwitchUnit
                    )
                }

                Spacer(Modifier.height(10.dp))

                ExerciseCardFooter(
                    state = state,
                    onNoteChange = onNoteChange,
                    onPinNote = onPinNote,
                    onRate = onRate,
                    onOpenSwapPicker = onOpenSwapPicker,
                    onToggleSkipped = onToggleSkipped
                )
            }
        }

        HorizontalDivider(color = outline.copy(alpha = 0.2f))
    }
}

