package com.forge.app.ui.gym.train.components

import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.animateIntAsState
import androidx.compose.foundation.Canvas
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
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
 * Live current-session readout + comparison sparkline. Left: elapsed time (ticking),
 * volume moved this exercise, and sets done — all updating as you train. Right: a small
 * dual line of this session's per-set volume (bright) over last session's (faint), so you
 * can see if you're on track to beat it. Tapping the strip opens the full chart detail.
 */
@Composable
internal fun LastSessionStrip(
    sessionStartedAtMs: Long?,
    currentVolumeLb: Double,
    currentSets: Int,
    targetSets: Int,
    currentVolumes: List<Double> = emptyList(),
    priorVolumes: List<Double> = emptyList(),
    onClick: () -> Unit = {}
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    // Count the volume up to its new total when a set lands, instead of snapping.
    val animatedVolume by animateIntAsState(targetValue = currentVolumeLb.toInt(), label = "volume")

    // Tick once a second so the elapsed time advances live.
    var nowMs by remember { mutableStateOf(System.currentTimeMillis()) }
    LaunchedEffect(sessionStartedAtMs) {
        while (true) {
            nowMs = System.currentTimeMillis()
            kotlinx.coroutines.delay(1000)
        }
    }
    val elapsedSec = sessionStartedAtMs?.let { ((nowMs - it) / 1000L).coerceAtLeast(0) } ?: 0L
    val elapsedStr = "%d:%02d".format(elapsedSec / 60, elapsedSec % 60)

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .border(0.5.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(6.dp))
            .clickable { onClick() }
            .padding(horizontal = 12.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(elapsedStr, style = MaterialTheme.typography.labelMedium, color = onBg, fontSize = 11.sp)
            Text("·", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.5f))
            Text("$animatedVolume lb", style = MaterialTheme.typography.labelMedium, color = onBg, fontSize = 11.sp)
            Text("·", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.5f))
            Text("$currentSets/$targetSets", style = MaterialTheme.typography.labelMedium, color = muted, fontSize = 11.sp)
        }
        Box(modifier = Modifier.weight(1f), contentAlignment = Alignment.CenterEnd) {
            // Only show the comparison line once you've logged a set this session —
            // a lone prior line over an empty session just looks like noise.
            if (currentVolumes.isNotEmpty()) {
                DualSparkline(
                    current = currentVolumes,
                    previous = priorVolumes,
                    currentColor = onBg,
                    previousColor = muted.copy(alpha = 0.75f),
                    modifier = Modifier.width(60.dp).height(16.dp)
                )
            }
        }
    }
}

/**
 * Two overlaid line series sharing one scale: [current] drawn bold, [previous] faint.
 * Single-point series render as a dot so an in-progress session still shows.
 */
@Composable
internal fun DualSparkline(
    current: List<Double>,
    previous: List<Double>,
    currentColor: Color,
    previousColor: Color,
    modifier: Modifier = Modifier
) {
    val all = current + previous
    if (all.isEmpty()) return
    val minV = all.min()
    val maxV = all.max()
    val range = (maxV - minV).coerceAtLeast(1.0)

    Canvas(modifier = modifier) {
        fun yFor(v: Double) = size.height - ((v - minV) / range * size.height).toFloat()
        fun drawSeries(values: List<Double>, color: Color, stroke: Float, dashed: Boolean) {
            if (values.isEmpty()) return
            if (values.size == 1) {
                drawCircle(color, radius = stroke + 1f, center = Offset(size.width / 2f, yFor(values[0])))
                return
            }
            val stepX = size.width / (values.size - 1)
            val path = Path()
            values.forEachIndexed { i, v ->
                val x = stepX * i
                val y = yFor(v)
                if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
            }
            val effect = if (dashed) androidx.compose.ui.graphics.PathEffect.dashPathEffect(floatArrayOf(6f, 5f)) else null
            drawPath(path, color = color, style = Stroke(width = stroke, pathEffect = effect))
        }
        // Last workout = dashed reference; current session = solid.
        drawSeries(previous, previousColor, 2f, dashed = true)
        drawSeries(current, currentColor, 3f, dashed = false)
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

/**
 * Completed-rest indicator shown between two logged sets — the actual time rested,
 * derived from the gap between their timestamps. Green = a finished rest period.
 */
@Composable
internal fun RestBetweenSets(seconds: Int) {
    if (seconds <= 0) return
    val readyColor = Color(0xFF5CB85C)
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val m = seconds / 60; val s = seconds % 60
    val label = if (m > 0) "$m:${"%02d".format(s)}" else "0:${"%02d".format(s)}"
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        HorizontalDivider(modifier = Modifier.weight(1f), color = readyColor.copy(alpha = 0.2f))
        Box(modifier = Modifier.size(7.dp).clip(CircleShape).background(readyColor))
        Text("rested $label", style = MaterialTheme.typography.labelSmall, color = readyColor.copy(alpha = 0.8f), fontSize = 9.sp)
        HorizontalDivider(modifier = Modifier.weight(1f), color = readyColor.copy(alpha = 0.2f))
    }
}

@Composable
internal fun InlineRestTimer(timer: RestTimerState, onTap: () -> Unit, onSkip: (() -> Unit)? = null) {
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
        // One-tap skip — the common "I'm ready, move on" action without opening a dialog.
        if (onSkip != null) {
            Text(
                if (isReady) "done" else "skip",
                style = MaterialTheme.typography.labelSmall,
                color = (if (isReady) readyColor else muted).copy(alpha = 0.8f),
                fontSize = 9.sp,
                letterSpacing = 1.sp,
                modifier = Modifier.clickable { onSkip() }.padding(start = 2.dp)
            )
        }
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

    Column(modifier = Modifier.fillMaxWidth().animateContentSize()) {
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
                // A quiet hint — visible but not competing with the exercise name.
                Box(modifier = Modifier.padding(horizontal = 4.dp)) {
                    Text(
                        "$delta ${if (delta.startsWith("+")) "↑" else "↓"}",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
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
