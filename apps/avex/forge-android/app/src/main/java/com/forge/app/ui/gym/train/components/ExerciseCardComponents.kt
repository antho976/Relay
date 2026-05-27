package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.timer.RestTimerState
import com.forge.app.ui.gym.train.state.ExerciseUiState

@Composable
internal fun CollapsedRow(
    exerciseIndex: Int,
    state: ExerciseUiState,
    isNow: Boolean,
    onToggle: () -> Unit,
    onOpenSwapPicker: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 14.dp),
        verticalAlignment = Alignment.Top
    ) {
        Text(
            "%02d".format(exerciseIndex + 1),
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontSize = 9.sp,
            modifier = Modifier.width(28.dp).padding(top = 5.dp)
        )
        Column(modifier = Modifier.weight(1f).clickable { onToggle() }) {
            val nameText = buildAnnotatedString {
                append(state.effectiveName)
                if (isNow) withStyle(SpanStyle(fontSize = 10.sp, color = muted)) { append("  ● NOW") }
            }
            Text(
                nameText,
                style = MaterialTheme.typography.headlineSmall,
                color = onBg,
                textDecoration = if (state.skipped) TextDecoration.LineThrough else TextDecoration.None
            )
            val priorLastSet = state.priorSets.lastOrNull()
            val lastPart = when {
                priorLastSet != null -> "  ·  last ${priorLastSet.weightText} × ${priorLastSet.reps}"
                else -> "  ·  never done"
            }
            Text(
                "${state.plan.sets} × ${state.plan.reps}  ·  ${state.plan.muscle.displayName.lowercase()}$lastPart",
                style = MaterialTheme.typography.bodySmall,
                color = muted,
                fontStyle = FontStyle.Italic
            )
            if (state.loggedSets.isNotEmpty()) {
                Spacer(Modifier.height(4.dp))
                Text(
                    "${state.loggedSets.size} / ${state.plan.sets}${if (state.wasPr) "  · PR" else ""}",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.primary,
                    fontSize = 10.sp,
                    fontWeight = FontWeight.SemiBold
                )
            }
        }
        Text(
            "⇌",
            style = MaterialTheme.typography.bodyLarge,
            color = muted.copy(alpha = 0.55f),
            modifier = Modifier.padding(start = 10.dp, top = 4.dp).clickable { onOpenSwapPicker() }
        )
    }
}

@Composable
internal fun ActionChip(prefix: String, label: String, onClick: () -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        modifier = Modifier.clickable { onClick() }.padding(horizontal = 8.dp, vertical = 4.dp)
    ) {
        Text(prefix, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
    }
}

@Composable
internal fun InlineRestTimer(timer: RestTimerState, onTap: () -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val readyColor = Color(0xFF5CB85C)
    val isReady = timer.isFinished
    val timeColor = if (isReady) readyColor else muted

    fun Int.toTimerLabel(): String {
        val m = this / 60; val s = this % 60
        return if (m > 0) "$m:${"%02d".format(s)}" else "%02d".format(s)
    }

    Row(
        modifier = Modifier.fillMaxWidth().clickable { onTap() }.padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Box(
            modifier = Modifier
                .size(10.dp)
                .clip(CircleShape)
                .border(1.dp, timeColor, CircleShape)
                .then(if (isReady) Modifier.background(readyColor) else Modifier),
            contentAlignment = Alignment.Center
        ) {}
        Text(if (isReady) "READY" else "REST", style = MaterialTheme.typography.labelSmall, color = timeColor, fontSize = 9.sp)
        Text(timer.secondsRemaining.toTimerLabel(), style = MaterialTheme.typography.labelLarge, color = timeColor)
        Text("/ ${timer.totalSeconds.toTimerLabel()}", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.45f), fontSize = 9.sp)
    }
}

@Composable
internal fun ExerciseCardFooter(
    state: ExerciseUiState,
    nextExerciseName: String?,
    onNoteChange: (String) -> Unit,
    onPinNote: (String) -> Unit,
    onRate: (EffortRating) -> Unit,
    onOpenSwapPicker: () -> Unit,
    onToggleSkipped: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .border(1.dp, outline.copy(alpha = 0.3f), RoundedCornerShape(6.dp))
            .padding(vertical = 12.dp),
        contentAlignment = Alignment.Center
    ) {
        Text("+ ADD A SET", style = MaterialTheme.typography.labelMedium, color = muted.copy(alpha = 0.6f))
    }

    Spacer(Modifier.height(16.dp))
    HorizontalDivider(color = outline.copy(alpha = 0.2f))
    Spacer(Modifier.height(12.dp))

    var showNote by remember(state.effectiveName) { mutableStateOf(!state.note.isNullOrBlank()) }
    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceAround) {
        ActionChip(prefix = "□", label = "NOTE") { showNote = !showNote }
        ActionChip(prefix = "⇌", label = "SWAP") { onOpenSwapPicker() }
        ActionChip(prefix = "⊳", label = if (state.skipped) "UN-SKIP" else "SKIP") { onToggleSkipped() }
    }

    if (showNote) {
        Spacer(Modifier.height(8.dp))
        NoteField(
            initialNote = state.note,
            onCommit = onNoteChange,
            onPinNote = { note -> onPinNote(note) },
            currentPinnedNote = state.pinnedNote,
            showTemplates = false
        )
    }

    var showRater by remember { mutableStateOf(state.difficulty != null) }
    Spacer(Modifier.height(8.dp))
    if (showRater || state.difficulty != null) {
        DifficultyRater(selected = state.difficulty, onSelect = onRate)
    } else {
        Text(
            "How did it feel?",
            style = MaterialTheme.typography.bodySmall,
            color = muted,
            fontStyle = FontStyle.Italic,
            modifier = Modifier.clickable { showRater = true }
        )
    }

    if (nextExerciseName != null) {
        Spacer(Modifier.height(12.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.2f))
        Row(
            modifier = Modifier.fillMaxWidth().padding(vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text("UP NEXT", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                Text(nextExerciseName, style = MaterialTheme.typography.bodyMedium, color = onBg.copy(alpha = 0.8f))
            }
            Text("→", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.4f))
        }
    }

    Spacer(Modifier.height(8.dp))
}
