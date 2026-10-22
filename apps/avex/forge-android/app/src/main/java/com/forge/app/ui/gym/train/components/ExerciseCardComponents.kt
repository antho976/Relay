package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.heightIn
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.animateIntAsState
import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.sizeIn
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
import android.content.Intent
import android.net.Uri
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
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
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.ROW_H
import androidx.compose.ui.unit.Dp
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
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeightDelta
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.program.ExerciseUnit
import com.forge.app.ui.gym.train.state.ExerciseUiState
import com.forge.app.ui.common.ForgeDisclosureCaret
import kotlin.math.roundToInt
import androidx.compose.foundation.layout.FlowRow

/**
 * A YouTube search for how to perform [exerciseName] — no per-exercise link curation needed.
 * [machine] biases the query toward cable/machine tutorials (the MWM-989 plate stations) so a
 * "Tricep Pushdown" or "Seated Bench Press" lands on machine demos, not barbell ones.
 */
internal fun youTubeSearchIntent(exerciseName: String, machine: Boolean = false): Intent {
    val query = if (machine) "$exerciseName cable machine exercise how to" else "$exerciseName exercise how to"
    return Intent(
        Intent.ACTION_VIEW,
        Uri.parse("https://www.youtube.com/results?search_query=" + Uri.encode(query))
    ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
}

/**
 * A prior set's load for the "last …" hints, as a bare number in the user's unit. Stored weight text
 * is always lb (toStoredWeightText), so printing it raw told a kg user their 100 kg was "220.5". A
 * plate machine's text is already a plate count, and a weightless set keeps its text ("BW").
 */
internal fun priorWeightLabel(
    set: com.forge.app.data.db.entities.LoggedSet,
    isPlates: Boolean,
    weightUnit: WeightUnit
): String {
    val lb = set.weightLb
    return if (isPlates || lb == null) set.weightText else weightInputValue(lb, weightUnit)
}

@Composable
internal fun CollapsedRow(
    exerciseIndex: Int,
    state: ExerciseUiState,
    isNow: Boolean,
    onToggle: () -> Unit,
    onOpenSwapPicker: () -> Unit,
    /** A group member passes its fill here and the row's gutter drops to the group's. */
    modifier: Modifier = Modifier,
    horizontalPadding: Dp = 24.dp
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val context = LocalContext.current
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = horizontalPadding, vertical = 14.dp),
        verticalAlignment = Alignment.Top
    ) {
        Text(
            "%02d".format(exerciseIndex + 1),
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontSize = 9.sp,
            modifier = Modifier.width(28.dp).padding(top = 5.dp)
        )
        Column(modifier = Modifier.weight(1f).clickableLabeled("Expand or collapse ${state.effectiveName}") { onToggle() }) {
            val nameText = buildAnnotatedString {
                append(state.effectiveName)
                if (isNow) withStyle(SpanStyle(fontSize = 10.sp, color = accent)) { append("  ● NOW") }
            }
            Text(
                nameText,
                style = MaterialTheme.typography.headlineSmall,
                color = onBg,
                textDecoration = if (state.skipped) TextDecoration.LineThrough else TextDecoration.None
            )
            val priorLastSet = state.priorSets.lastOrNull()
            val weightUnit = LocalForgeSettings.current.weightUnit
            val lastPart = when {
                priorLastSet != null ->
                    "  ·  last ${priorWeightLabel(priorLastSet, state.effectiveUnit == ExerciseUnit.PLATES, weightUnit)} × ${priorLastSet.reps}"
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
        // Trailing actions — demo (YouTube) + swap, both 18dp icons so they line up.
        Row(
            modifier = Modifier.padding(start = 8.dp, top = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            // Swap-aware, like the name beside it: the demo search should describe the movement the
            // card is actually showing, not the one the slot started as.
            val machine = state.effectiveUnit == ExerciseUnit.PLATES
            // 18dp glyphs on a 48dp touch target (§14): the icon centers inside the larger box.
            Box(
                modifier = Modifier.size(48.dp)
                    .clickableLabeled("Watch a demo of ${state.effectiveName}") {
                        runCatching { context.startActivity(youTubeSearchIntent(state.effectiveName, machine)) }
                    },
                contentAlignment = Alignment.Center
            ) {
                Icon(TrainIcons.Video, contentDescription = "Watch a demo",
                    tint = muted.copy(alpha = 0.65f), modifier = Modifier.size(18.dp))
            }
            Box(
                modifier = Modifier.size(48.dp).clickableLabeled("Swap ${state.effectiveName}") { onOpenSwapPicker() },
                contentAlignment = Alignment.Center
            ) {
                Icon(TrainIcons.Swap, contentDescription = "Swap",
                    tint = muted.copy(alpha = 0.65f), modifier = Modifier.size(18.dp))
            }
        }
    }
}

/**
 * Session progress under the exercise counter: one cell per exercise of the day, accent for the
 * ones handled (done, skipped, or the one you are on), hollow for the ones ahead.
 *
 * §2② — a filled / hollow rail for a set of items, some present — and the same geometry as
 * onboarding's `StepRail`. Written locally rather than promoted to `ui/common`: this is the
 * pattern's SECOND screen, and §8 promotes on the third.
 *
 * It is also where this screen's accent starts. A session used to spend its accent only on the
 * commit button at the very bottom, which left the colour reading as a sticker rather than as part
 * of a system; anchoring the same accent at the top of the card gives it a span to belong to.
 */
@Composable
internal fun SessionRail(done: List<Boolean>, currentIndex: Int, modifier: Modifier = Modifier) {
    if (done.size < 2) return
    val accent = MaterialTheme.colorScheme.primary
    val track = MaterialTheme.colorScheme.outline.copy(alpha = 0.25f)
    Row(
        modifier
            .fillMaxWidth()
            .semantics {
                contentDescription =
                    "Exercise ${currentIndex + 1} of ${done.size}, ${done.count { it }} done"
            },
        horizontalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        done.forEachIndexed { i, isDone ->
            val color by animateColorAsState(
                if (isDone || i == currentIndex) accent else track,
                ForgeMotion.standardTween(),
                label = "session_rail_cell"
            )
            Box(
                Modifier
                    .weight(1f)
                    .height(3.dp)
                    .clip(RoundedCornerShape(50))
                    .background(color)
            )
        }
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
    val weightUnit = LocalForgeSettings.current.weightUnit
    // Count the volume up to its new total when a set lands, instead of snapping. Rounded, not
    // truncated, like formatVolume: 28.6 × 8 is 228.8 and used to read "228 lb".
    val volume = toDisplayWeight(currentVolumeLb, weightUnit)
    val animatedVolume by animateIntAsState(targetValue = if (volume.isFinite()) volume.roundToInt() else 0, label = "volume")

    // Tick once a second so the elapsed time advances live — but only when a session has
    // actually started. Without the guard the loop recomposes the strip every second even
    // when elapsed is pinned at 0:00 (no session start).
    var nowMs by remember { mutableStateOf(System.currentTimeMillis()) }
    LaunchedEffect(sessionStartedAtMs) {
        if (sessionStartedAtMs == null) return@LaunchedEffect
        while (true) {
            nowMs = System.currentTimeMillis()
            kotlinx.coroutines.delay(1000)
        }
    }
    val elapsedSec = sessionStartedAtMs?.let { ((nowMs - it) / 1000L).coerceAtLeast(0) } ?: 0L
    val elapsedStr = workoutClock(elapsedSec)
    val unit = unitLabel(weightUnit)

    // Filled, not outlined: the raised fill is what says "tap for the chart".
    val stripModifier = Modifier
        .fillMaxWidth()
        .clip(RoundedCornerShape(12.dp))
        .background(MaterialTheme.colorScheme.surfaceContainerHigh)
        .clickableLabeled("Open this exercise's chart") { onClick() }
        // One reading for TalkBack: the figures are bare ("0 lb"), so it says what each one is.
        .clearAndSetSemantics {
            contentDescription = "Workout time $elapsedStr, $animatedVolume $unit lifted, $currentSets of $targetSets sets"
        }
        .padding(horizontal = 14.dp, vertical = 11.dp)
    // A FlowRow, so at large text the figures wrap between items, never inside "3 of 4 sets".
    val figures: @Composable () -> Unit = {
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
            itemVerticalAlignment = Alignment.CenterVertically
        ) {
            Text(elapsedStr, style = MaterialTheme.typography.labelMedium, color = onBg)
            Text("·", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.65f))
            Text("$animatedVolume $unit", style = MaterialTheme.typography.labelMedium, color = onBg)
            Text("·", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.65f))
            Text("$currentSets of $targetSets sets", style = MaterialTheme.typography.labelMedium, color = muted)
        }
    }
    // Only show the comparison line once you've logged a set this session —
    // a lone prior line over an empty session just looks like noise.
    val sparkline: @Composable () -> Unit = {
        DualSparkline(
            current = currentVolumes,
            previous = priorVolumes,
            currentColor = MaterialTheme.colorScheme.primary,
            previousColor = muted.copy(alpha = 0.65f),
            modifier = Modifier.width(60.dp).height(16.dp)
        )
    }
    if (SetTable.stacked()) {
        // At large text the figures need the whole width; the line gets its own row under them
        // instead of being squeezed to a few pixels beside them.
        Column(stripModifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
            figures()
            if (currentVolumes.isNotEmpty()) sparkline()
        }
    } else {
        Row(
            stripModifier,
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Box(Modifier.weight(1f)) { figures() }
            if (currentVolumes.isNotEmpty()) sparkline()
        }
    }
}

/**
 * A workout's elapsed time: "5:12", then "1:05:12" past the hour (it read "75:12"). Digits are
 * padded by hand, not with "%02d".format, which writes the device locale's digits beside ASCII ones.
 */
internal fun workoutClock(seconds: Long): String = com.forge.app.domain.units.clockText(seconds)

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
        modifier = Modifier.heightIn(min = 48.dp).clickableLabeled(label) { onClick() }.padding(horizontal = 8.dp, vertical = 4.dp)
    ) {
        Icon(icon, contentDescription = label, tint = muted, modifier = Modifier.size(15.dp))
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
    }
}

/**
 * Completed-rest indicator shown between two logged sets — the actual time rested,
 * derived from the gap between their timestamps. Green = a finished rest period.
 */
/**
 * The rest taken between two consecutive sets of one exercise, in seconds, or null when the gap was
 * not a rest worth printing.
 *
 * The label used to print the raw gap between the two LOG taps, which is not the rest: it also holds
 * the whole next set (and the typing), so a 2:00 rest before a 45-second set read "rested 2:50", and
 * a gap with another exercise's set inside it (a superset, or jumping around the gym) read as one
 * long rest of 6 or 7 minutes. Now the next set's own time comes off (its measured hold, else the
 * same 45 seconds the session estimate uses), a gap with other work logged inside it isn't called a
 * rest at all, and neither is one past the ten minutes the rest tuner already treats as walking off.
 */
internal fun restBetweenSeconds(
    prev: com.forge.app.data.db.entities.LoggedSet,
    next: com.forge.app.data.db.entities.LoggedSet,
    /** When every OTHER exercise's sets this session were logged. */
    otherSetTimes: List<Long> = emptyList()
): Int? {
    // Seeded / imported rows can carry no real time.
    if (prev.completedAt <= 0L || next.completedAt <= prev.completedAt) return null
    if (otherSetTimes.any { it > prev.completedAt && it < next.completedAt }) return null
    val gap = ((next.completedAt - prev.completedAt) / 1000L).toInt()
    val work = next.durationSeconds?.takeIf { it > 0 } ?: com.forge.app.program.SessionEstimate.WORK_SECONDS_PER_SET
    val rest = gap - work
    return rest.takeIf { it in 1..MAX_SHOWN_REST_SECONDS }
}

/** Past this, the gap was not a rest (matches AdaptThresholds.maxSaneRestSeconds). */
private const val MAX_SHOWN_REST_SECONDS = 600

@Composable
internal fun RestBetweenSets(seconds: Int) {
    if (seconds <= 0) return
    val readyColor = ForgeLastGreen
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val label = com.forge.app.domain.units.clockText(seconds.toLong())
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        HorizontalDivider(modifier = Modifier.weight(1f), color = readyColor.copy(alpha = 0.25f))
        Box(modifier = Modifier.size(7.dp).clip(CircleShape).background(readyColor))
        Text("rested $label", style = MaterialTheme.typography.labelSmall, color = readyColor, fontSize = 9.sp)
        HorizontalDivider(modifier = Modifier.weight(1f), color = readyColor.copy(alpha = 0.25f))
    }
}

@Composable
internal fun InlineRestTimer(timer: RestTimerState, onTap: () -> Unit, onSkip: (() -> Unit)? = null) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val readyColor = ForgeLastGreen
    val isReady = timer.isFinished
    val timeColor = if (isReady) readyColor else muted

    // Under a minute it reads bare seconds ("45"), as it always has; the digits are padded by
    // hand so a locale with its own numerals doesn't mix them into a clock.
    fun Int.toTimerLabel(): String =
        if (this >= 60) com.forge.app.domain.units.clockText(toLong()) else toString().padStart(2, '0')

    Row(
        modifier = Modifier.fillMaxWidth().clickableLabeled("Open the rest timer") { onTap() }.padding(vertical = 4.dp),
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
            Text("/ ${timer.totalSeconds.toTimerLabel()}", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.65f), fontSize = 9.sp)
        }
        HorizontalDivider(modifier = Modifier.weight(1f), color = muted.copy(alpha = 0.25f))
        // One-tap end-rest — the common "I'm ready, move on" action without opening a dialog.
        // Always "done" (you finish the rest normally, not "skip" it).
        // A 48dp target of its own: as 9sp text with 2dp of padding inside a row whose tap opens the
        // controls, a near miss opened the dialog instead of ending the rest.
        if (onSkip != null) {
            Box(
                modifier = Modifier
                    .sizeIn(minWidth = 48.dp, minHeight = 48.dp)
                    .clickableLabeled("End rest") { onSkip() },
                contentAlignment = Alignment.Center
            ) {
                Text(
                    "done",
                    style = MaterialTheme.typography.labelSmall,
                    color = if (isReady) readyColor else muted,
                    fontSize = 9.sp,
                    letterSpacing = 1.sp
                )
            }
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

    // Air, not a rule, between the set log and the footer.
    Spacer(Modifier.height(16.dp))

    val context = LocalContext.current
    val showNote = remember(state.effectiveName) { !state.note.isNullOrBlank() }
    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceAround) {
        // No NOTE here: the top bar's note sits beside Finish, and two note buttons read as one
        // too many (Antho, 2026-10-21). A note already on the exercise still opens below.
        ActionChip(icon = TrainIcons.Video, label = "VIDEO") {
            runCatching { context.startActivity(youTubeSearchIntent(state.effectiveName, state.effectiveUnit == ExerciseUnit.PLATES)) }
        }
        ActionChip(icon = TrainIcons.Swap, label = "SWAP") { onOpenSwapPicker() }
        ActionChip(icon = TrainIcons.Skip, label = if (state.skipped) "UN-SKIP" else "SKIP") { onToggleSkipped() }
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
    Row(
        modifier = Modifier.clickableLabeled(
            if (showRater) "Hide the effort rating" else "Rate the effort"
        ) { showRater = !showRater },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        Text(
            "How did it feel?",
            style = MaterialTheme.typography.bodySmall,
            color = muted,
            fontStyle = FontStyle.Italic
        )
        ForgeDisclosureCaret(showRater, tint = muted)
    }
    if (showRater) {
        Spacer(Modifier.height(8.dp))
        DifficultyRater(selected = state.difficulty, onSelect = onRate)
    }

    Spacer(Modifier.height(8.dp))
}

/**
 * The UP NEXT pill's text for a suggested change of [deltaLb] (stored lb): signed, in the user's
 * unit (a plate count on PLATES exercises), with the ↑/↓ outside the number. Null below half a
 * pound, which is kg and plate rounding noise rather than a change.
 *
 * The pill used to print the raw pound Double ("+2.299999999999997 ↑" for a kg user, "+15" for one
 * plate), pre-formatted in the ViewModel with no unit (audit 2026-09-26, DESIGN §11).
 */
internal fun upNextDeltaLabel(deltaLb: Double?, isPlates: Boolean, weightUnit: WeightUnit, plateLb: Double): String? {
    if (deltaLb == null || !deltaLb.isFinite() || kotlin.math.abs(deltaLb) < 0.5) return null
    val magnitude = kotlin.math.abs(deltaLb)
    val amount = if (isPlates && plateLb > 0.0) "${formatPlateCount(magnitude / plateLb)} pl"
        else formatWeightDelta(magnitude, weightUnit)
    return if (deltaLb > 0) "+$amount ↑" else "−$amount ↓"
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
    /** The next exercise's own suggested change, in stored lb — formatted here, in the user's unit. */
    nextDeltaLb: Double?,
    nextIsPlates: Boolean,
    upcoming: List<Pair<Int, ExerciseUiState>>,
    onSelectExercise: (String) -> Unit,
    onOpenSwapPicker: (String) -> Unit,
    onAddExercise: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    var expanded by remember { mutableStateOf(false) }

    // One group: the header, then (expanded) each upcoming exercise and the add row as members
    // joined by 2dp seams, instead of an outlined box over a hairline-split list.
    val memberCount = if (expanded) upcoming.size + 2 else 1
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp)
            .animateContentSize(),
        verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)
    ) {
        // Header — always visible, toggles the list.
        Row(
            modifier = Modifier
                .groupMember(0, memberCount)
                .clickableLabeled(if (expanded) "Hide what's up next" else "Show what's up next") { expanded = !expanded }
                .padding(horizontal = ROW_H, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text("UP NEXT", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                val label = buildString {
                    append(nextName ?: "Last exercise · finish or add another")
                    // Non-breaking inside the target, so large text breaks at the "·" rather than
                    // leaving "Lat Pulldown · 3" over "× 6-10".
                    if (!nextTarget.isNullOrBlank()) append(" · ${nextTarget.replace(' ', '\u00A0')}")
                }
                Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
            }
            val settings = LocalForgeSettings.current
            upNextDeltaLabel(nextDeltaLb, nextIsPlates, settings.weightUnit, settings.plateWeightLb)?.let { delta ->
                // A quiet hint — visible but not competing with the exercise name.
                Box(modifier = Modifier.padding(horizontal = 4.dp)) {
                    Text(
                        delta,
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 10.sp
                    )
                }
            }
            ForgeDisclosureCaret(expanded, tint = muted)
        }

        if (expanded) {
            upcoming.forEachIndexed { i, (idx, ex) ->
                CollapsedRow(
                    exerciseIndex = idx,
                    state = ex,
                    isNow = false,
                    onToggle = { onSelectExercise(ex.plan.id); expanded = false },
                    onOpenSwapPicker = { onOpenSwapPicker(ex.plan.id) },
                    modifier = Modifier.groupMember(i + 1, memberCount),
                    horizontalPadding = ROW_H
                )
            }
            Text(
                "+ add exercise",
                style = MaterialTheme.typography.bodyMedium,
                color = muted,
                modifier = Modifier
                    .groupMember(memberCount - 1, memberCount)
                    .clickableLabeled("Add an exercise") { onAddExercise() }
                    .padding(horizontal = ROW_H, vertical = 16.dp)
            )
        }
    }
}
