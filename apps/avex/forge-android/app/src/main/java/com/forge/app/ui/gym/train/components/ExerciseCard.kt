package com.forge.app.ui.gym.train.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
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
import com.forge.app.ui.gym.train.state.ExerciseUiState

@Composable
fun ExerciseCard(
    state: ExerciseUiState,
    onToggle: () -> Unit,
    onLogSet: (weightText: String, reps: Int) -> Unit,
    onDeleteSet: (setId: Long) -> Unit,
    onLogSameAsLast: (setId: Long) -> Unit,
    onRate: (EffortRating) -> Unit,
    onNoteChange: (String) -> Unit,
    onToggleSkipped: () -> Unit,
    onOpenSwapPicker: () -> Unit,
    modifier: Modifier = Modifier
) {
    val cardAlpha = if (state.skipped) 0.45f else 1f

    Surface(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .alpha(cardAlpha),
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 2.dp
    ) {
        Column {
            Header(state = state, onToggle = onToggle, onOpenSwapPicker = onOpenSwapPicker)

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
                            onLongPress = { onLogSameAsLast(set.id) }
                        )
                    }

                    if (!state.skipped) {
                        SetInputRow(
                            prefillWeight = state.prefillWeight,
                            suggestedWeight = state.suggestedWeight,
                            suggestionReason = state.suggestionReason,
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
                        onCommit = onNoteChange
                    )

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.End
                    ) {
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
    onOpenSwapPicker: () -> Unit
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
            }
            Text(
                "${state.plan.sets} × ${state.plan.reps}  ·  ${state.plan.muscle.displayName}",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            if (state.loggedSets.isNotEmpty()) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
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
                }
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
