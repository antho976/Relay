@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.gym.freestyle

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.MAX_REPS_DIGITS
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.filterDecimalInput
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.ForgeChoiceChip
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeRowPill
import com.forge.app.ui.common.GlyphButton
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.rirLabel
import com.forge.app.ui.common.rpeLabel
import com.forge.app.ui.gym.stats.components.MuscleFigure
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.ForgeMotion

/** Font scale at which the two number fields stop sharing a line (a 28sp serif field at 2x cannot). */
private const val STACK_AT_FONT_SCALE = 1.5f

// ── Session header ─────────────────────────────────────────────────────────────────────────────

/**
 * The top of the log: a mono eyebrow carrying the session clock and the rest since the last logged
 * set, then either the first-run line or three live serif figures (exercises · sets · volume).
 */
@Composable
internal fun FsSessionHeader(
    elapsedMs: Long,
    restMs: Long?,
    exerciseCount: Int,
    setCount: Int,
    volumeLb: Double,
    weightUnit: WeightUnit
) {
    val cs = MaterialTheme.colorScheme
    Column(Modifier.fillMaxWidth().padding(bottom = 8.dp)) {
        val rest = restMs?.let { " · REST ${formatElapsed(it)}" }.orEmpty()
        Text(
            "FREESTYLE · ${formatElapsed(elapsedMs)}$rest",
            style = MaterialTheme.typography.labelMedium,
            color = cs.onSurfaceVariant,
            letterSpacing = 1.sp
        )
        Spacer(Modifier.height(10.dp))
        if (exerciseCount == 0) {
            Text("Log as you go", style = MaterialTheme.typography.headlineMedium, color = cs.onSurface)
            Spacer(Modifier.height(6.dp))
            Text(
                "Add a move, then log each set as you finish it. Numbers carry over from your last set, so a repeat is one tap.",
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurfaceVariant
            )
        } else {
            Row(Modifier.fillMaxWidth()) {
                FsFigure(exerciseCount.toString(), if (exerciseCount == 1) "EXERCISE" else "EXERCISES", Modifier.weight(1f))
                FsFigure(setCount.toString(), if (setCount == 1) "SET" else "SETS", Modifier.weight(1f))
                FsFigure(
                    headerVolume(volumeLb, weightUnit),
                    "VOLUME · ${weightUnit.label.uppercase()}",
                    Modifier.weight(1.3f)
                )
            }
        }
    }
}

/** The header's volume figure: exact to one decimal under 1,000 so it matches the cards, compact above. */
private fun headerVolume(volumeLb: Double, unit: WeightUnit): String = when {
    volumeLb <= 0 -> "0"
    com.forge.app.domain.units.toDisplayWeight(volumeLb, unit) < 1000 -> weightInputValue(volumeLb, unit)
    else -> formatVolumeCompact(volumeLb, unit, withUnit = false)
}

@Composable
private fun FsFigure(value: String, label: String, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Column(modifier.semantics(mergeDescendants = true) {}) {
        Text(value, style = MaterialTheme.typography.headlineMedium, color = cs.onSurface)
        Text(label, style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant)
    }
}

/** An unsaved log is waiting: pick it up or start over. Nothing else renders until this is settled. */
@Composable
internal fun FsResumePrompt(exerciseCount: Int, setCount: Int, onResume: () -> Unit, onStartFresh: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Column(Modifier.fillMaxWidth()) {
        Text("UNSAVED", style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant, letterSpacing = 1.sp)
        Spacer(Modifier.height(10.dp))
        Text("Unfinished workout", style = MaterialTheme.typography.headlineMedium, color = cs.onSurface)
        Spacer(Modifier.height(6.dp))
        val ex = "$exerciseCount ${if (exerciseCount == 1) "exercise" else "exercises"}"
        val sets = "$setCount ${if (setCount == 1) "set" else "sets"}"
        Text("$ex and $sets, logged and not saved.", style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
        Spacer(Modifier.height(20.dp))
        ForgePrimaryCapsule("Resume log", onClick = onResume, modifier = Modifier.fillMaxWidth())
        Spacer(Modifier.height(8.dp))
        ForgeOutlineCapsule("Start fresh", onClick = onStartFresh, modifier = Modifier.fillMaxWidth())
    }
}

// ── Exercise card ──────────────────────────────────────────────────────────────────────────────

/** Name row shared by the folded and open card: muscle thumbnail, name, one mono meta line. */
@Composable
private fun FsExerciseTitle(
    exercise: FsExercise,
    meta: String,
    modifier: Modifier = Modifier,
    trailing: @Composable () -> Unit = {}
) {
    val cs = MaterialTheme.colorScheme
    Row(modifier.fillMaxWidth().heightIn(min = 56.dp), verticalAlignment = Alignment.CenterVertically) {
        MuscleFigure(
            muscle = exercise.muscle,
            lit = lerp(cs.primary, cs.onSurface, 0.32f),
            body = cs.onSurfaceVariant.copy(alpha = 0.15f),
            detail = cs.onSurfaceVariant.copy(alpha = 0.35f),
            modifier = Modifier.width(40.dp).height(44.dp)
        )
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(exercise.name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, color = cs.onSurface)
            Text(meta, style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant)
        }
        trailing()
    }
}

/**
 * A folded exercise: one tappable row that reads as its own summary (sets and top set), so a long
 * workout stays scannable. Tap opens it for logging; hold drags it (the drag wrapper owns that).
 */
@Composable
internal fun FsFoldedCard(
    exercise: FsExercise,
    unitLabel: String,
    weightUnit: WeightUnit,
    dragging: Boolean,
    onOpen: () -> Unit
) {
    val cs = MaterialTheme.colorScheme
    val n = exercise.sets.size
    val meta = buildString {
        append(exercise.muscle.displayName.uppercase())
        append(" · ")
        if (n == 0) append("NO SETS YET")
        else {
            append("$n ${if (n == 1) "SET" else "SETS"}")
            exercise.topSet(weightUnit)?.let { append(" · TOP ${exercise.setReading(it, unitLabel)}") }
        }
    }
    Box(
        Modifier.fillMaxWidth()
            .padding(top = 12.dp)
            .clip(RoundedCornerShape(12.dp))
            .then(if (dragging) Modifier.background(cs.surfaceVariant) else Modifier)
            .bounceClick(onClick = onOpen)
            .semantics { stateDescription = "Folded. Tap to log sets" }
    ) {
        FsExerciseTitle(exercise, meta) { ForgeRowPill("Log") }
    }
}

/**
 * The open exercise: its logged sets as a ledger, last time's numbers, and the entry slab that logs
 * the next set. Only one exercise is open at a time, which keeps the thumb on one slab.
 */
@Composable
internal fun FsOpenCard(
    exercise: FsExercise,
    entry: FsEntry,
    lastTime: List<LoggedSet>,
    pinnedNote: String,
    unitLabel: String,
    weightUnit: WeightUnit,
    dragging: Boolean,
    tagsOpen: Boolean,
    stopwatchStartMs: Long?,
    nowMs: Long,
    onFold: () -> Unit,
    onRemove: () -> Unit,
    onEntryChange: (FsSet) -> Unit,
    onToggleTags: () -> Unit,
    onLog: () -> Unit,
    onEditSet: (Int) -> Unit,
    onCancelEdit: () -> Unit,
    onDeleteSet: (Int) -> Unit,
    onRepeatLastTime: () -> Unit,
    onUseLastSet: (LoggedSet) -> Unit,
    onToggleStopwatch: () -> Unit
) {
    val cs = MaterialTheme.colorScheme
    val bestLast = lastTimeBestLb(lastTime)
    val volume = exercise.volumeLb(weightUnit)
    val n = exercise.sets.size
    val meta = buildString {
        append(exercise.muscle.displayName.uppercase())
        append(" · $n ${if (n == 1) "SET" else "SETS"}")
        if (volume > 0) append(" · ${formatWeight(volume, weightUnit)}")
    }
    Column(
        Modifier.fillMaxWidth()
            .padding(top = 16.dp)
            .clip(RoundedCornerShape(12.dp))
            .then(if (dragging) Modifier.background(cs.surfaceVariant) else Modifier)
            .animateContentSize(ForgeMotion.snappy())
    ) {
        FsExerciseTitle(
            exercise,
            meta,
            Modifier.clickableLabeled("Fold ${exercise.name}", onClick = onFold)
        ) {
            GlyphButton("×", "Remove ${exercise.name}", cs.onSurfaceVariant, onClick = onRemove)
        }

        if (pinnedNote.isNotBlank()) {
            Text(
                "“$pinnedNote”",
                style = MaterialTheme.typography.bodySmall,
                color = cs.onSurfaceVariant,
                fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(start = 52.dp, bottom = 4.dp)
            )
        }

        if (lastTime.isNotEmpty()) {
            FsLastTimeRow(
                lastTime = lastTime,
                timed = exercise.timed,
                weightUnit = weightUnit,
                showRepeat = exercise.sets.isEmpty(),
                onUse = onUseLastSet,
                onRepeat = onRepeatLastTime
            )
        }

        if (exercise.sets.isNotEmpty()) {
            Spacer(Modifier.height(4.dp))
            exercise.sets.forEachIndexed { i, set ->
                FsLedgerRow(
                    index = i,
                    reading = exercise.setReading(set, unitLabel),
                    tags = set.tagLabels(),
                    beatsLast = exercise.beatsLastTime(set, bestLast, weightUnit),
                    editing = entry.editing == i,
                    onClick = { if (entry.editing == i) onCancelEdit() else onEditSet(i) }
                )
            }
        }

        Spacer(Modifier.height(10.dp))
        FsEntrySlab(
            exercise = exercise,
            entry = entry,
            unitLabel = unitLabel,
            weightUnit = weightUnit,
            tagsOpen = tagsOpen,
            stopwatchElapsedSec = stopwatchStartMs?.let { ((nowMs - it) / 1000).toInt().coerceAtLeast(0) },
            onChange = onEntryChange,
            onToggleTags = onToggleTags,
            onLog = onLog,
            onCancelEdit = onCancelEdit,
            onDelete = { entry.editing?.let(onDeleteSet) },
            onToggleStopwatch = onToggleStopwatch
        )
        Spacer(Modifier.height(8.dp))
    }
}

/**
 * Last time's sets as tappable readings (tap one to load it into the slab), plus Repeat all on an
 * exercise with nothing logged yet. One wrapping line, so it costs the card a single row.
 */
@Composable
private fun FsLastTimeRow(
    lastTime: List<LoggedSet>,
    timed: Boolean,
    weightUnit: WeightUnit,
    showRepeat: Boolean,
    onUse: (LoggedSet) -> Unit,
    onRepeat: () -> Unit
) {
    val cs = MaterialTheme.colorScheme
    FlowRow(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        itemVerticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            "LAST",
            style = MaterialTheme.typography.labelSmall,
            color = cs.onSurfaceVariant,
            modifier = Modifier.padding(end = 4.dp)
        )
        lastTime.forEach { s ->
            val reading = s.compactReading(timed, weightUnit)
            Box(
                Modifier
                    .minimumInteractiveComponentSize()
                    .clickableLabeled("Use $reading") { onUse(s) },
                contentAlignment = Alignment.Center
            ) {
                Text(
                    reading,
                    style = MaterialTheme.typography.labelMedium,
                    color = cs.onSurface,
                    modifier = Modifier
                        .border(1.dp, cs.outline.copy(alpha = 0.35f), RoundedCornerShape(50))
                        .padding(horizontal = 10.dp, vertical = 5.dp)
                )
            }
        }
        if (showRepeat) {
            Text(
                "Repeat all \u2192",
                style = MaterialTheme.typography.labelLarge,
                color = cs.primary,
                modifier = Modifier
                    .minimumInteractiveComponentSize()
                    .clickableLabeled("Log every set from last time", onClick = onRepeat)
                    .padding(horizontal = 6.dp)
            )
        }
    }
}

/** One logged set: number, reading, tags, and the beat-last-time mark. Tap to edit it in the slab. */
@Composable
private fun FsLedgerRow(
    index: Int,
    reading: String,
    tags: List<String>,
    beatsLast: Boolean,
    editing: Boolean,
    onClick: () -> Unit
) {
    val cs = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth()
            .heightIn(min = 48.dp)
            .clip(RoundedCornerShape(8.dp))
            .then(if (editing) Modifier.background(cs.primary.copy(alpha = 0.15f)) else Modifier)
            .clickableLabeled(if (editing) "Stop editing set ${index + 1}" else "Edit set ${index + 1}", onClick = onClick)
            .padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text("%02d".format(index + 1), style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant, modifier = Modifier.width(32.dp))
        Text(reading, style = MaterialTheme.typography.titleLarge, color = cs.onSurface)
        if (tags.isNotEmpty()) {
            Spacer(Modifier.width(10.dp))
            Text(tags.joinToString("  "), style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant)
        }
        Spacer(Modifier.weight(1f))
        if (beatsLast) {
            Text(
                "△ LAST",
                style = MaterialTheme.typography.labelSmall,
                color = ForgeLastGreen,
                modifier = Modifier.semantics { contentDescription = "Beats last time" }
            )
        }
    }
}

// ── Entry slab ─────────────────────────────────────────────────────────────────────────────────

/**
 * The one place a set is typed: big serif numbers with steppers, an optional tag tray, and the Log
 * button. It starts filled (see [seedEntry]), so for a straight set the whole job is one tap. When a
 * logged set is tapped the slab edits that set instead and the button reads Update.
 */
@Composable
private fun FsEntrySlab(
    exercise: FsExercise,
    entry: FsEntry,
    unitLabel: String,
    weightUnit: WeightUnit,
    tagsOpen: Boolean,
    stopwatchElapsedSec: Int?,
    onChange: (FsSet) -> Unit,
    onToggleTags: () -> Unit,
    onLog: () -> Unit,
    onCancelEdit: () -> Unit,
    onDelete: () -> Unit,
    onToggleStopwatch: () -> Unit
) {
    val cs = MaterialTheme.colorScheme
    val set = entry.set
    val editing = entry.editing
    val stacked = LocalDensity.current.fontScale >= STACK_AT_FONT_SCALE
    val secondFocus = androidx.compose.runtime.remember { FocusRequester() }
    val canLog = exercise.isLogged(
        if (exercise.timed && stopwatchElapsedSec != null) set.copy(hold = holdText(stopwatchElapsedSec)) else set
    )
    val step = weightStepFor(weightUnit)
    val showWeight = !exercise.bodyweight

    Column(
        Modifier.fillMaxWidth()
            .background(cs.surfaceVariant, RoundedCornerShape(12.dp))
            .padding(horizontal = 12.dp, vertical = 12.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                if (editing != null) "EDITING SET %02d".format(editing + 1) else "SET %02d".format(exercise.sets.size + 1),
                style = MaterialTheme.typography.labelMedium,
                color = if (editing != null) cs.primary else cs.onSurfaceVariant,
                letterSpacing = 1.sp,
                modifier = Modifier.weight(1f)
            )
            if (exercise.bodyweight && !exercise.timed) {
                Text("BODYWEIGHT", style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant)
            }
        }
        Spacer(Modifier.height(8.dp))

        val weightField: @Composable (Modifier) -> Unit = { m ->
            FsStepperField(
                value = set.weight,
                label = unitLabel.uppercase(),
                placeholder = "0",
                keyboardType = KeyboardType.Decimal,
                imeAction = ImeAction.Next,
                onImeAction = { secondFocus.requestFocus() },
                onValueChange = { onChange(set.copy(weight = filterDecimalInput(it).take(6))) },
                onMinus = { onChange(set.copy(weight = stepWeightText(set.weight, -step))) },
                onPlus = { onChange(set.copy(weight = stepWeightText(set.weight, step))) },
                what = "weight",
                modifier = m
            )
        }
        val secondField: @Composable (Modifier) -> Unit = { m ->
            if (exercise.timed) {
                FsStepperField(
                    value = stopwatchElapsedSec?.let { holdText(it) } ?: set.hold,
                    label = "HOLD",
                    placeholder = "0:00",
                    keyboardType = KeyboardType.Number,
                    imeAction = ImeAction.Done,
                    onImeAction = { if (canLog) onLog() },
                    onValueChange = { onChange(set.copy(hold = sanitizeHoldText(it))) },
                    onMinus = { onChange(set.copy(hold = stepHoldText(set.hold, -5))) },
                    onPlus = { onChange(set.copy(hold = stepHoldText(set.hold, 5))) },
                    what = "hold time by 5 seconds",
                    enabled = stopwatchElapsedSec == null,
                    focusRequester = secondFocus,
                    modifier = m
                )
            } else {
                FsStepperField(
                    value = set.reps,
                    label = "REPS",
                    placeholder = "0",
                    keyboardType = KeyboardType.Number,
                    imeAction = ImeAction.Done,
                    onImeAction = { if (canLog) onLog() },
                    // Capped: an 11-digit entry overflows Int, toIntOrNull returns null, and the set
                    // could never be logged, with no error shown.
                    onValueChange = { new -> onChange(set.copy(reps = new.filter { it.isDigit() }.take(MAX_REPS_DIGITS))) },
                    onMinus = { onChange(set.copy(reps = stepRepsText(set.reps, -1))) },
                    onPlus = { onChange(set.copy(reps = stepRepsText(set.reps, 1))) },
                    what = "reps",
                    focusRequester = secondFocus,
                    modifier = m
                )
            }
        }

        if (showWeight && !stacked) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                weightField(Modifier.weight(1f))
                secondField(Modifier.weight(1f))
            }
        } else {
            if (showWeight) {
                weightField(Modifier.fillMaxWidth())
                Spacer(Modifier.height(8.dp))
            }
            secondField(Modifier.fillMaxWidth())
        }

        if (exercise.timed) {
            Spacer(Modifier.height(4.dp))
            Text(
                if (stopwatchElapsedSec != null) "Stop timer" else "Start timer",
                style = MaterialTheme.typography.labelLarge,
                color = cs.primary,
                modifier = Modifier
                    .align(Alignment.CenterHorizontally)
                    .minimumInteractiveComponentSize()
                    .clickableLabeled(if (stopwatchElapsedSec != null) "Stop the hold timer" else "Start the hold timer", onClick = onToggleStopwatch)
                    .padding(horizontal = 8.dp)
            )
        }

        // Tags tray: closed by default so the numbers stay the whole job. The toggle names what is set.
        val summary = set.tagLabels()
        Row(
            Modifier.fillMaxWidth()
                .heightIn(min = 48.dp)
                .clickableLabeled(if (tagsOpen) "Hide set tags" else "Show set tags", onClick = onToggleTags),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                if (summary.isEmpty()) "Tags" else "Tags · ${summary.joinToString(" ")}",
                style = MaterialTheme.typography.labelLarge,
                color = if (summary.isEmpty()) cs.onSurfaceVariant else cs.onSurface,
                modifier = Modifier.weight(1f)
            )
            Text(if (tagsOpen) "−" else "+", style = MaterialTheme.typography.titleMedium, color = cs.onSurfaceVariant)
        }
        AnimatedVisibility(
            visible = tagsOpen,
            enter = expandVertically(ForgeMotion.enterTween()) + fadeIn(ForgeMotion.enterTween()),
            exit = shrinkVertically(ForgeMotion.exitTween()) + fadeOut(ForgeMotion.exitTween())
        ) {
            FsTagTray(set = set, onChange = onChange)
        }

        Spacer(Modifier.height(8.dp))
        if (editing != null) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ForgeOutlineCapsule("Delete", onClick = onDelete, contentColor = cs.error, modifier = Modifier.weight(1f))
                ForgeOutlineCapsule("Cancel", onClick = onCancelEdit, modifier = Modifier.weight(1f))
            }
            Spacer(Modifier.height(8.dp))
            ForgePrimaryCapsule("Update set", onClick = onLog, enabled = canLog, accent = true, modifier = Modifier.fillMaxWidth())
        } else {
            ForgePrimaryCapsule("Log set", onClick = onLog, enabled = canLog, accent = true, modifier = Modifier.fillMaxWidth())
        }
    }
}

/**
 * A number with its steppers: − and + either side of a big serif field, the unit or field name
 * under it. Steppers nudge without opening the keyboard, which is the fast path between sets.
 */
@Composable
private fun FsStepperField(
    value: String,
    label: String,
    placeholder: String,
    keyboardType: KeyboardType,
    imeAction: ImeAction,
    onImeAction: () -> Unit,
    onValueChange: (String) -> Unit,
    onMinus: () -> Unit,
    onPlus: () -> Unit,
    what: String,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    focusRequester: FocusRequester? = null
) {
    val cs = MaterialTheme.colorScheme
    Column(modifier, horizontalAlignment = Alignment.CenterHorizontally) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            FsStepButton("−", "Decrease $what", enabled, onMinus)
            val line = cs.outline
            BasicTextField(
                value = value,
                onValueChange = onValueChange,
                enabled = enabled,
                textStyle = MaterialTheme.typography.headlineMedium.copy(color = cs.onSurface, textAlign = TextAlign.Center),
                singleLine = true,
                cursorBrush = SolidColor(cs.primary),
                keyboardOptions = KeyboardOptions(keyboardType = keyboardType, imeAction = imeAction),
                keyboardActions = KeyboardActions(onNext = { onImeAction() }, onDone = { onImeAction() }),
                modifier = Modifier
                    .weight(1f)
                    .then(if (focusRequester != null) Modifier.focusRequester(focusRequester) else Modifier)
                    .semantics { contentDescription = label.lowercase() }
                    .drawBehind {
                        val y = size.height - 1.dp.toPx()
                        drawLine(line, Offset(0f, y), Offset(size.width, y), strokeWidth = 1.dp.toPx())
                    },
                decorationBox = { inner ->
                    Box(Modifier.fillMaxWidth().padding(bottom = 4.dp), contentAlignment = Alignment.Center) {
                        if (value.isEmpty()) {
                            Text(
                                placeholder,
                                style = MaterialTheme.typography.headlineMedium,
                                color = cs.onSurfaceVariant.copy(alpha = 0.35f)
                            )
                        }
                        inner()
                    }
                }
            )
            FsStepButton("+", "Increase $what", enabled, onPlus)
        }
        Spacer(Modifier.height(4.dp))
        Text(label, style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant)
    }
}

@Composable
private fun FsStepButton(symbol: String, label: String, enabled: Boolean, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Box(
        Modifier
            .sizeIn(minWidth = 44.dp, minHeight = 48.dp)
            .clip(RoundedCornerShape(50))
            .bounceClick(enabled = enabled, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center
    ) {
        Text(
            symbol,
            style = MaterialTheme.typography.headlineSmall,
            color = if (enabled) cs.onSurface else cs.onSurface.copy(alpha = 0.35f)
        )
    }
}

/**
 * Per-set tags (GYMAP-46): the mutually-exclusive shape, the independent AMRAP and failure flags,
 * and effort as RPE 6 to 10. Each edit flows straight back through [onChange].
 */
@Composable
private fun FsTagTray(set: FsSet, onChange: (FsSet) -> Unit) {
    val cs = MaterialTheme.colorScheme
    val rpeOptions = generateSequence(6.0) { it + 0.5 }.takeWhile { it <= 10.0 }.toList()
    Column(Modifier.fillMaxWidth().padding(bottom = 4.dp)) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            ForgeChoiceChip("Warm-up", selected = set.setType == "warmup", onClick = {
                onChange(set.copy(setType = if (set.setType == "warmup") null else "warmup"))
            })
            ForgeChoiceChip("Drop set", selected = set.setType == "drop", onClick = {
                onChange(set.copy(setType = if (set.setType == "drop") null else "drop"))
            })
            ForgeChoiceChip("AMRAP", selected = set.isAmrap, onClick = { onChange(set.copy(isAmrap = !set.isAmrap)) })
            ForgeChoiceChip("To failure", selected = set.toFailure, onClick = { onChange(set.copy(toFailure = !set.toFailure)) })
        }
        Spacer(Modifier.height(6.dp))
        val rpe = set.rpe
        Text(
            if (rpe == null) "EFFORT · RPE" else "EFFORT · RPE ${rpeLabel(rpe)} · ${rirLabel(rpe)} IN RESERVE",
            style = MaterialTheme.typography.labelSmall,
            color = cs.onSurfaceVariant
        )
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            rpeOptions.forEach { v ->
                val selected = rpe != null && kotlin.math.abs(rpe - v) < 0.01
                ForgeChoiceChip(
                    rpeLabel(v),
                    selected = selected,
                    onClick = { onChange(set.copy(rpe = if (selected) null else v)) }
                )
            }
        }
    }
}

// ── Footer ─────────────────────────────────────────────────────────────────────────────────────

/**
 * Below the exercises: the add action (the page's hero while the log is empty), one-tap chips for
 * recently performed moves, and the reuse-a-workout entry on an empty log.
 */
@Composable
internal fun FsAddFooter(
    empty: Boolean,
    recent: List<Pair<String, String>>,
    showTemplates: Boolean,
    onAdd: () -> Unit,
    onQuickAdd: (String) -> Unit,
    onTemplates: () -> Unit
) {
    val cs = MaterialTheme.colorScheme
    Column(Modifier.fillMaxWidth().padding(top = 24.dp, bottom = 16.dp)) {
        if (empty) ForgePrimaryCapsule("Add exercise", onClick = onAdd, modifier = Modifier.fillMaxWidth())
        else ForgeOutlineCapsule("Add exercise", onClick = onAdd, modifier = Modifier.fillMaxWidth())
        if (showTemplates) {
            Spacer(Modifier.height(8.dp))
            ForgeOutlineCapsule("Start from a past workout", onClick = onTemplates, modifier = Modifier.fillMaxWidth())
        }
        if (recent.isNotEmpty()) {
            Spacer(Modifier.height(20.dp))
            Text("RECENT · TAP TO ADD", style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, letterSpacing = 1.sp)
            Spacer(Modifier.height(4.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                recent.forEach { (id, name) ->
                    ForgeChoiceChip("+ $name", selected = false, onClick = { onQuickAdd(id) })
                }
            }
        }
    }
}
