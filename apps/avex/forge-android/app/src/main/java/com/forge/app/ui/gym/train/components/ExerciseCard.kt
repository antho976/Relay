package com.forge.app.ui.gym.train.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.types.EffortRating
import com.forge.app.ui.gym.train.state.ExerciseUiState

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ExerciseCard(
    exerciseIndex: Int,
    state: ExerciseUiState,
    isNow: Boolean = false,
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
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    val cardAlpha = if (state.skipped) 0.45f else 1f
    val longPressModifier = if (onLongPress != null)
        Modifier.combinedClickable(onClick = {}, onLongClick = onLongPress)
    else Modifier

    var showRater by remember { mutableStateOf(state.difficulty != null) }

    val priorLastSet = state.priorSets.lastOrNull()

    Column(
        modifier = modifier
            .fillMaxWidth()
            .alpha(cardAlpha)
            .then(longPressModifier)
    ) {
        // ── Header ──────────────────────────────────────────────────────────
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp, vertical = 14.dp),
            verticalAlignment = Alignment.Top
        ) {
            // Number — fixed width, nudged to align with name baseline
            Text(
                String.format("%02d", exerciseIndex + 1),
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 9.sp,
                modifier = Modifier
                    .width(28.dp)
                    .padding(top = 5.dp)
            )

            // Name + subtitle (tappable)
            Column(
                modifier = Modifier
                    .weight(1f)
                    .clickable { onToggle() }
            ) {
                val nameText = buildAnnotatedString {
                    append(state.effectiveName)
                    if (isNow) {
                        withStyle(SpanStyle(fontSize = 10.sp, color = muted)) {
                            append("  ● NOW")
                        }
                    }
                }
                Text(
                    nameText,
                    style = MaterialTheme.typography.headlineSmall,
                    color = onBg,
                    textDecoration = if (state.skipped) TextDecoration.LineThrough else TextDecoration.None
                )

                // Subtitle: sets × reps · muscle · last X × Y / never done (when collapsed)
                val lastPart = when {
                    !state.isExpanded && priorLastSet != null ->
                        "  ·  last ${priorLastSet.weightText} × ${priorLastSet.reps}"
                    !state.isExpanded -> "  ·  never done"
                    else -> ""
                }
                Text(
                    "${state.plan.sets} × ${state.plan.reps}  ·  ${state.plan.muscle.displayName.lowercase()}$lastPart",
                    style = MaterialTheme.typography.bodySmall,
                    color = muted,
                    fontStyle = FontStyle.Italic
                )
                state.allTimePbText?.let { pb ->
                    Text(
                        "PB $pb",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted.copy(alpha = 0.5f),
                        fontSize = 9.sp
                    )
                }

                if (state.loggedSets.isNotEmpty()) {
                    Spacer(Modifier.height(4.dp))
                    Text(
                        "${state.loggedSets.size} / ${state.plan.sets}${if (state.wasPr) "  · PR" else ""}",
                        style = MaterialTheme.typography.labelSmall,
                        color = accent,
                        fontSize = 10.sp,
                        fontWeight = FontWeight.SemiBold
                    )
                }
            }

            // Swap shortcut (collapsed only)
            if (!state.isExpanded) {
                Text(
                    "⇌",
                    style = MaterialTheme.typography.bodyLarge,
                    color = muted.copy(alpha = 0.55f),
                    modifier = Modifier
                        .padding(start = 10.dp, top = 4.dp)
                        .clickable { onOpenSwapPicker() }
                )
            }
        }

        // ── Expanded body ────────────────────────────────────────────────────
        AnimatedVisibility(visible = state.isExpanded) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 24.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                HorizontalDivider(color = outline.copy(alpha = 0.3f))

                priorLastSet?.let { last ->
                    Text(
                        "Last session you did ${last.weightText} × ${last.reps}.",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted,
                        fontStyle = FontStyle.Italic
                    )
                }

                // SETS: horizontal display with remaining "—" slots
                if (state.loggedSets.isNotEmpty()) {
                    Text(
                        "SETS",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp
                    )
                    Spacer(Modifier.height(2.dp))
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(20.dp),
                        modifier = Modifier.horizontalScroll(rememberScrollState())
                    ) {
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
                        val remaining = maxOf(0, state.plan.sets - state.loggedSets.size)
                        repeat(remaining) {
                            Text(
                                "—",
                                style = MaterialTheme.typography.headlineSmall,
                                color = muted.copy(alpha = 0.2f)
                            )
                        }
                    }
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

                if (showRater || state.difficulty != null) {
                    DifficultyRater(
                        selected = state.difficulty,
                        onSelect = onRate
                    )
                } else {
                    Text(
                        "How did it feel?",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted,
                        fontStyle = FontStyle.Italic,
                        modifier = Modifier.clickable { showRater = true }
                    )
                }

                var showNote by remember(state.effectiveName) { mutableStateOf(!state.note.isNullOrBlank()) }
                if (showNote) {
                    NoteField(
                        initialNote = state.note,
                        onCommit = onNoteChange,
                        onPinNote = { note -> onPinNote(note) },
                        currentPinnedNote = state.pinnedNote,
                        showTemplates = false
                    )
                } else {
                    Text(
                        "+ note",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontStyle = FontStyle.Italic,
                        modifier = Modifier.clickable { showNote = true }
                    )
                }

                // Footer: skip exercise | swap →
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        if (state.skipped) "un-skip" else "skip exercise",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontStyle = FontStyle.Italic,
                        modifier = Modifier.clickable { onToggleSkipped() }
                    )
                    Text(
                        "swap →",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontStyle = FontStyle.Italic,
                        modifier = Modifier.clickable { onOpenSwapPicker() }
                    )
                }
                Spacer(Modifier.height(4.dp))
            }
        }

        HorizontalDivider(color = outline.copy(alpha = 0.2f))
    }
}
