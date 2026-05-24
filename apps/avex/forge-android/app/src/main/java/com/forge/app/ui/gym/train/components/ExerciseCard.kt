package com.forge.app.ui.gym.train.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.types.EffortRating
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.gym.train.state.ExerciseUiState
import com.forge.app.ui.gym.train.state.VsLastStatus

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ExerciseCard(
    state: ExerciseUiState,
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
    onMoveUp: (() -> Unit)? = null,
    onMoveDown: (() -> Unit)? = null,
    onLongPress: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    val cardAlpha = if (state.skipped) 0.45f else 1f
    val longPressModifier = if (onLongPress != null)
        Modifier.combinedClickable(onClick = {}, onLongClick = onLongPress)
    else Modifier

    Surface(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .alpha(cardAlpha)
            .then(longPressModifier),
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 2.dp
    ) {
        Column {
            Header(state = state, onToggle = onToggle, onOpenSwapPicker = onOpenSwapPicker,
                onOpenGoalSetter = onOpenGoalSetter, onOpenRestTimerSetter = onOpenRestTimerSetter,
                onMoveUp = onMoveUp, onMoveDown = onMoveDown)

            AnimatedVisibility(visible = state.isExpanded) {
                Column(
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.4f))

                    state.lastSessionPreviewText?.let { preview ->
                        Text(
                            preview,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }

                    state.loggedSets.forEach { set ->
                        SetRow(
                            set = set,
                            isPr = set.id in state.prSetIds,
                            onDelete = { onDeleteSet(set.id) },
                            onEdit = { w, r -> onEditSet(set.id, w, r) },
                            onLongPress = { onLogSameAsLast(set.id) },
                            onToggleDifficultyTag = { tag -> onToggleSetDifficultyTag(set.id, tag) }
                        )
                    }

                    if (!state.skipped) {
                        SetInputRow(
                            prefillWeight = state.prefillWeight,
                            suggestedWeight = state.suggestedWeight,
                            suggestionReason = state.suggestionReason,
                            priorSets = state.priorSets,
                            onSubmit = onLogSet
                        )
                    }

                    if (state.loggedSets.isNotEmpty()) {
                        Text(
                            "How was it?",
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        DifficultyRater(
                            selected = state.difficulty,
                            onSelect = onRate
                        )
                    }

                    NoteField(
                        initialNote = state.note,
                        onCommit = onNoteChange,
                        onPinNote = { note -> onPinNote(note) },
                        currentPinnedNote = state.pinnedNote
                    )

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.End
                    ) {
                        // Per-exercise unit memory (#70): only shown for dumbbell exercises
                        if (state.plan.unit.name == "DUMBBELL") {
                            val globalKg = LocalForgeSettings.current.useKg
                            val exerciseUnit = state.persistentSwapUnit?.takeIf { it.isNotBlank() }
                            val showsKg = exerciseUnit == "kg" || (exerciseUnit == null && globalKg)
                            TextButton(onClick = { onSetExerciseUnit(if (showsKg) "lb" else "kg") }) {
                                Text("Unit: ${if (showsKg) "kg" else "lb"}")
                            }
                        }
                        TextButton(onClick = onOpenRestTimerSetter) {
                            val label = state.restTimerOverrideSeconds?.let { "${it / 60}m ${it % 60}s rest" } ?: "Set rest timer"
                            Text(label)
                        }
                        TextButton(onClick = onOpenGoalSetter) {
                            Text(if (state.goalWeightLb != null) "Update goal" else "Set goal")
                        }
                        TextButton(onClick = onToggleSkipped) {
                            Text(if (state.skipped) "Un-skip" else "Skip exercise")
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun Header(
    state: ExerciseUiState,
    onToggle: () -> Unit,
    onOpenSwapPicker: () -> Unit,
    onOpenGoalSetter: () -> Unit,
    onOpenRestTimerSetter: () -> Unit,
    onMoveUp: (() -> Unit)? = null,
    onMoveDown: (() -> Unit)? = null
) {
    val accent = MaterialTheme.colorScheme.primary
    val isSwapped = state.sessionSwapName != null || state.persistentSwapName != null

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onToggle() }
            .padding(start = 16.dp, end = 8.dp, top = 16.dp, bottom = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Column(Modifier.weight(1f)) {
            // Exercise name row
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(
                    state.effectiveName,
                    style = MaterialTheme.typography.titleLarge,
                    color = MaterialTheme.colorScheme.onSurface,
                    textDecoration = if (state.skipped) TextDecoration.LineThrough else TextDecoration.None
                )
                if (isSwapped) {
                    Text(
                        "(swap)",
                        style = MaterialTheme.typography.labelLarge,
                        color = accent,
                        fontWeight = FontWeight.SemiBold
                    )
                }
                state.supersetGroup?.let {
                    Text(
                        "SS",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.tertiary,
                        fontWeight = FontWeight.Black
                    )
                }
            }

            // Form cue chip (#8) — shown on first set of first occurrence if cue is non-null
            state.plan.formCue?.let { cue ->
                if (state.loggedSets.isEmpty()) {
                    Text(
                        "💡 $cue",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.tertiary
                    )
                }
            }

            // Pinned note always visible in header (#112)
            if (state.pinnedNote.isNotBlank()) {
                Text(
                    "📌 ${state.pinnedNote}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.primary,
                    fontWeight = FontWeight.SemiBold
                )
            }

            // Muscle / sets plan row + PB text (#101)
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Text(
                    "${state.plan.sets} × ${state.plan.reps}  ·  ${state.plan.muscle.displayName}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                state.allTimePbText?.let { pb ->
                    Text(
                        "· PB: $pb",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
                    )
                }
            }

            // Goal weight progress bar (#28)
            state.goalWeightLb?.let { goal ->
                val frac = state.goalProgressFraction ?: 0f
                val pct = (frac * 100).toInt()
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    LinearProgressIndicator(
                        progress = { frac },
                        modifier = Modifier.weight(1f).height(3.dp).clip(RoundedCornerShape(2.dp)),
                        color = MaterialTheme.colorScheme.primary,
                        trackColor = MaterialTheme.colorScheme.surfaceVariant
                    )
                    Text(
                        "Goal ${goal.toInt()} lb · $pct%",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.primary
                    )
                }
            }

            // Sets-logged progress + PR badge + vs-last status chip (#102, #104)
            if (state.loggedSets.isNotEmpty()) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Text(
                        "${state.loggedSets.size} / ${state.plan.sets} sets logged",
                        style = MaterialTheme.typography.labelLarge,
                        color = accent
                    )
                    if (state.wasPr) {
                        Text(
                            "· PR",
                            style = MaterialTheme.typography.labelLarge,
                            color = accent,
                            fontWeight = FontWeight.Black
                        )
                    }
                    state.vsLastStatus?.let { status ->
                        val (label, color) = when (status) {
                            VsLastStatus.BEATING -> "↑ Beating" to MaterialTheme.colorScheme.primary
                            VsLastStatus.MATCHING -> "= Matching" to MaterialTheme.colorScheme.onSurfaceVariant
                            VsLastStatus.UNDER -> "↓ Under" to MaterialTheme.colorScheme.error
                        }
                        Text(
                            label,
                            style = MaterialTheme.typography.labelSmall,
                            color = color
                        )
                    }
                }
            }
        }
        if (onMoveUp != null) {
            IconButton(onClick = onMoveUp, modifier = Modifier.size(32.dp)) {
                Icon(Icons.Default.KeyboardArrowUp, contentDescription = "Move up",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        if (onMoveDown != null) {
            IconButton(onClick = onMoveDown, modifier = Modifier.size(32.dp)) {
                Icon(Icons.Default.KeyboardArrowDown, contentDescription = "Move down",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        IconButton(onClick = onOpenSwapPicker) {
            Icon(
                Icons.Default.SwapHoriz,
                contentDescription = "Swap exercise",
                tint = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        Icon(
            imageVector = if (state.isExpanded) Icons.Default.ExpandLess else Icons.Default.ExpandMore,
            contentDescription = if (state.isExpanded) "Collapse" else "Expand",
            tint = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}
