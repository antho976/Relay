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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.outlined.SkipNext
import androidx.compose.material.icons.outlined.SwapHoriz
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
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
import androidx.compose.ui.graphics.vector.ImageVector
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
import com.forge.app.ui.gym.stats.components.Sparkline
import com.forge.app.ui.gym.train.state.ExerciseSessionPoint
import com.forge.app.ui.gym.train.state.ExerciseUiState
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

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
                    "${state.loggedSets.size} / ${state.targetSets}${if (state.wasPr) "  · PR" else ""}",
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

/**
 * Last-session metadata strip + sparkline. Shows the most recent prior session's
 * date · duration · this-exercise volume, with a small line chart of top-weight-per-session
 * spanning the last ~8 sessions.
 */
@Composable
internal fun LastSessionStrip(history: List<ExerciseSessionPoint>) {
    if (history.isEmpty()) return
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline

    val last = history.first() // newest first
    val zone = ZoneId.systemDefault()
    val dateStr = Instant.ofEpochMilli(last.sessionStartedAt)
        .atZone(zone)
        .format(DateTimeFormatter.ofPattern("MM/dd", Locale.getDefault()))
    val durationStr = last.durationMin?.let { "$it MIN" }
    val volumeStr = last.volumeLb.takeIf { it > 0 }?.let { "${it.toInt()} LB" }

    val sparkValues = history.asReversed().mapNotNull { it.topWeightLb }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .border(0.5.dp, outline.copy(alpha = 0.3f), RoundedCornerShape(4.dp))
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(dateStr, style = MaterialTheme.typography.labelSmall, color = onBg, fontSize = 10.sp)
            if (durationStr != null) {
                Text("·", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.4f))
                Text(durationStr, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
            }
            if (volumeStr != null) {
                Text("·", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.4f))
                Text(volumeStr, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
            }
        }
        Box(modifier = Modifier.weight(1f), contentAlignment = Alignment.CenterEnd) {
            if (sparkValues.size >= 2) {
                Sparkline(
                    values = sparkValues,
                    lineColor = onBg.copy(alpha = 0.55f),
                    minValue = sparkValues.min(),
                    maxValue = sparkValues.max(),
                    modifier = Modifier.width(56.dp).height(14.dp)
                )
            }
        }
    }
}

@Composable
internal fun ActionChip(icon: ImageVector, label: String, onClick: () -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        modifier = Modifier.clickable { onClick() }.padding(horizontal = 8.dp, vertical = 4.dp)
    ) {
        Icon(icon, contentDescription = label, tint = muted, modifier = Modifier.size(15.dp))
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
        modifier = Modifier.fillMaxWidth().clickable { onTap() }.padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        HorizontalDivider(modifier = Modifier.weight(1f), color = muted.copy(alpha = 0.25f))
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Box(
                modifier = Modifier
                    .size(10.dp)
                    .clip(CircleShape)
                    .border(1.dp, timeColor, CircleShape)
                    .then(if (isReady) Modifier.background(readyColor) else Modifier),
                contentAlignment = Alignment.Center
            ) {}
            Text(if (isReady) "READY" else "REST", style = MaterialTheme.typography.labelSmall, color = timeColor, fontSize = 9.sp, letterSpacing = 1.sp)
            Text(timer.secondsRemaining.toTimerLabel(), style = MaterialTheme.typography.labelLarge, color = timeColor)
            Text("/ ${timer.totalSeconds.toTimerLabel()}", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.45f), fontSize = 9.sp)
        }
        HorizontalDivider(modifier = Modifier.weight(1f), color = muted.copy(alpha = 0.25f))
    }
}

@Composable
internal fun ExerciseCardFooter(
    state: ExerciseUiState,
    onNoteChange: (String) -> Unit,
    onPinNote: (String) -> Unit,
    onRate: (EffortRating) -> Unit,
    onOpenSwapPicker: () -> Unit,
    onToggleSkipped: () -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline

    Spacer(Modifier.height(8.dp))
    HorizontalDivider(color = outline.copy(alpha = 0.2f))
    Spacer(Modifier.height(12.dp))

    var showNote by remember(state.effectiveName) { mutableStateOf(!state.note.isNullOrBlank()) }
    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceAround) {
        ActionChip(icon = Icons.Outlined.Description, label = "NOTE") { showNote = !showNote }
        ActionChip(icon = Icons.Outlined.SwapHoriz, label = "SWAP") { onOpenSwapPicker() }
        ActionChip(icon = Icons.Outlined.SkipNext, label = if (state.skipped) "UN-SKIP" else "SKIP") { onToggleSkipped() }
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
    Text(
        if (showRater) "How did it feel?  ▲" else "How did it feel?  ▾",
        style = MaterialTheme.typography.bodySmall,
        color = muted,
        fontStyle = FontStyle.Italic,
        modifier = Modifier.clickable { showRater = !showRater }
    )
    if (showRater) {
        Spacer(Modifier.height(8.dp))
        DifficultyRater(selected = state.difficulty, onSelect = onRate)
    }

    Spacer(Modifier.height(8.dp))
}

/**
 * Standalone "UP NEXT" bubble shown below the current exercise in the single-exercise
 * train view. Collapsed it shows the next exercise + target + suggested-weight delta pill;
 * tapping expands it to the full list of upcoming exercises (each tappable to jump to it).
 */
@Composable
internal fun UpNextBubble(
    nextName: String?,
    nextTarget: String?,
    nextDelta: String?,
    upcoming: List<Pair<Int, ExerciseUiState>>,
    onSelectExercise: (String) -> Unit,
    onOpenSwapPicker: (String) -> Unit,
    onAddExercise: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    var expanded by remember { mutableStateOf(false) }

    Column(modifier = Modifier.fillMaxWidth()) {
        // Collapsed header — always visible, toggles the list.
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp)
                .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(16.dp))
                .clickable { if (upcoming.isNotEmpty()) expanded = !expanded }
                .padding(horizontal = 14.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Text("☰", style = MaterialTheme.typography.labelMedium, color = muted.copy(alpha = 0.6f))
            Column(modifier = Modifier.weight(1f)) {
                Text("UP NEXT", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                val label = buildString {
                    append(nextName ?: "Last exercise — hit FINISH")
                    if (!nextTarget.isNullOrBlank()) append(" · $nextTarget")
                }
                Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
            }
            nextDelta?.let { delta ->
                val isPositive = delta.startsWith("+")
                val pillColor = if (isPositive) Color(0xFF5CB85C) else muted
                Box(
                    modifier = Modifier
                        .border(0.5.dp, pillColor.copy(alpha = 0.5f), RoundedCornerShape(50))
                        .padding(horizontal = 10.dp, vertical = 3.dp)
                ) {
                    Text(
                        "$delta ${if (isPositive) "↑" else "↓"}",
                        style = MaterialTheme.typography.labelSmall,
                        color = pillColor,
                        fontSize = 10.sp
                    )
                }
            }
            if (upcoming.isNotEmpty()) {
                Text(if (expanded) "▲" else "▾", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.5f))
            }
        }

        if (expanded) {
            Spacer(Modifier.height(8.dp))
            upcoming.forEach { (idx, ex) ->
                CollapsedRow(
                    exerciseIndex = idx,
                    state = ex,
                    isNow = false,
                    onToggle = { onSelectExercise(ex.plan.id); expanded = false },
                    onOpenSwapPicker = { onOpenSwapPicker(ex.plan.id) }
                )
                HorizontalDivider(
                    modifier = Modifier.padding(horizontal = 24.dp),
                    color = outline.copy(alpha = 0.12f)
                )
            }
            Text(
                "+ add exercise",
                style = MaterialTheme.typography.bodyMedium,
                color = muted,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onAddExercise() }
                    .padding(horizontal = 24.dp, vertical = 16.dp)
            )
        }
    }
}
