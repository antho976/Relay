package com.forge.app.ui.gym.train.components

import androidx.compose.animation.animateContentSize
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
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
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
import com.forge.app.program.ExerciseUnit
import com.forge.app.ui.common.rpeLabel
import com.forge.app.ui.gym.train.state.ExerciseUiState

/** Reps to PRE-FILL the field with — numeric targets only ("8-12" → 12, "15" → 15); null otherwise. */
private val TARGET_REPS_REGEX = Regex("""^(\d+)(?:-(\d+))?$""")
private fun targetRepsOf(reps: String): Int? {
    val m = TARGET_REPS_REGEX.matchEntire(reps.trim()) ?: return null
    return m.groupValues[2].ifEmpty { m.groupValues[1] }.toIntOrNull()
}

/**
 * A rep number to SHOW as the field's greyed hint even when it starts empty: numeric → its top
 * ("8-12" → 12), per-leg → the count ("10/leg" → 10), AMRAP → a sensible 12 to aim for. Timed
 * holds (e.g. "30-60s") have no rep count, so no hint.
 */
private fun recommendedRepsOf(reps: String): Int? {
    val t = reps.trim()
    if (t.equals("AMRAP", ignoreCase = true)) return 12
    if (t.contains('s')) return null
    return Regex("""\d+""").findAll(t).map { it.value.toInt() }.lastOrNull()
}

/** One italic "Suggested next → …" cue line, with an optional parenthesised reason. Shared by the
 *  weighted suggestion and the bodyweight rep-progression cue so the two can't drift in styling (CO5). */
@Composable
private fun SuggestionLine(label: String, reason: String?, muted: Color) {
    Spacer(Modifier.height(2.dp))
    val line = buildAnnotatedString {
        withStyle(SpanStyle(fontStyle = FontStyle.Italic)) {
            append(label)
            if (!reason.isNullOrBlank()) append(" ($reason)")
        }
    }
    Text(line, style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.75f))
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ExerciseCard(
    exerciseIndex: Int,
    state: ExerciseUiState,
    isNow: Boolean = false,
    totalExercises: Int = 0,
    restTimerState: RestTimerState? = null,
    sessionStartedAtMs: Long? = null,
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
    onSkipRest: () -> Unit = {},
    onSetExerciseUnit: (String?) -> Unit = {},
    onPinNote: (String) -> Unit = {},
    onToggleSetDifficultyTag: (setId: Long, currentTag: String?) -> Unit = { _, _ -> },
    onSetRpe: (setId: Long, rpe: Double?) -> Unit = { _, _ -> },
    onAddSet: () -> Unit = {},
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
    // onLongClickLabel keeps the quick-actions menu discoverable for TalkBack now that the visible
    // "⋯ options" affordance is gone — the gesture is announced even though there's no chrome for it.
    val longPressModifier = if (onLongPress != null)
        Modifier.combinedClickable(
            onClick = {},
            onLongClickLabel = "Exercise options — skip, swap, or set the rest timer",
            onLongClick = onLongPress
        )
    else Modifier

    Column(
        modifier = modifier
            .fillMaxWidth()
            .animateContentSize()
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

                val isBodyweight = state.plan.unit == ExerciseUnit.BODYWEIGHT
                val isPlates = state.plan.unit == ExerciseUnit.PLATES

                // Exercise counter. The "⋯ options" button was removed (its actions — swap via the
                // name, skip/rate via the footer, rest timer via the timer bubble — are all reachable
                // directly); the quick-actions menu still opens on a long-press of the card.
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
                                append(" @ RPE ${rpeLabel(rpe)}")
                            }
                        } else {
                            append(" · first time — no history yet")
                        }
                    }
                }
                Text(targetText, style = MaterialTheme.typography.bodySmall, color = muted)

                // Suggested next line — a weight (weighted lifts) or, for bodyweight, more reps (CO5).
                if (state.suggestedWeight != null) {
                    SuggestionLine("Suggested next → ${state.suggestedWeight}", state.suggestionReason, muted)
                }
                if (state.suggestedReps != null) {
                    SuggestionLine("Suggested next → ${state.suggestedReps} reps", state.suggestedRepsReason, muted)
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

                // Form cue — read-only tip from the exercise definition, shown small + muted.
                // Suppressed when the slot is swapped: plan.formCue describes the base movement, not
                // the swapped-in exercise, so it would be actively wrong coaching (#11).
                val formCue = state.plan.formCue.takeUnless { state.isSwapped }
                if (!formCue.isNullOrBlank()) {
                    Spacer(Modifier.height(4.dp))
                    Text(
                        "ⓘ $formCue",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted.copy(alpha = 0.55f),
                        fontStyle = FontStyle.Italic
                    )
                }

                // Per-set volume is derived three ways below; memoize so the rest-timer's
                // per-second ticks (which recompose this card) don't re-walk the set list.
                val currentVolumes = remember(state.loggedSets) {
                    state.loggedSets.map { (it.weightLb ?: 0.0) * it.reps }
                }
                val currentVolumeLb = remember(currentVolumes) { currentVolumes.sum() }
                val priorVolumes = remember(state.priorSets) {
                    state.priorSets.map { (it.weightLb ?: 0.0) * it.reps }
                }

                // Live current-session readout + per-set volume comparison (current vs last).
                Spacer(Modifier.height(12.dp))
                LastSessionStrip(
                    sessionStartedAtMs = sessionStartedAtMs,
                    currentVolumeLb = currentVolumeLb,
                    currentSets = state.loggedSets.size,
                    targetSets = state.targetSets,
                    currentVolumes = currentVolumes,
                    priorVolumes = priorVolumes,
                    onClick = onOpenChart
                )

                Spacer(Modifier.height(16.dp))

                // Session stats chip (sets progress + volume)
                if (state.loggedSets.isNotEmpty()) {
                    val volumeText = if (currentVolumeLb > 0) "  ·  ${currentVolumeLb.toInt()} LB" else ""
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
                    Text(
                        when { isBodyweight -> "BODYWEIGHT"; isPlates -> "PLATES"; else -> "WEIGHT · LB" },
                        style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.weight(1f)
                    )
                    Text("REPS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.width(48.dp))
                    Text("RPE", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.width(44.dp), textAlign = TextAlign.Center)
                    Text("LAST", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.width(72.dp), textAlign = TextAlign.End)
                }
                HorizontalDivider(color = outline.copy(alpha = 0.3f), modifier = Modifier.padding(top = 4.dp))

                // Logged set rows, with the actual rest taken shown between consecutive sets
                state.loggedSets.forEachIndexed { i, set ->
                    key(set.id) {
                        SetRow(
                            set = set,
                            setIndex = i + 1,
                            isPr = set.id == state.bestPrSetId,
                            isPlates = isPlates,
                            priorSet = state.priorSets.getOrNull(i),
                            priorFallbackSet = state.priorSets.lastOrNull(),
                            onDelete = { onDeleteSet(set.id) },
                            onEdit = { w, r -> onEditSet(set.id, w, r) },
                            onLongPress = { onLogSameAsLast(set.id) },
                            onToggleDifficultyTag = { tag -> onToggleSetDifficultyTag(set.id, tag) },
                            onSetRpe = { rpe -> onSetRpe(set.id, rpe) }
                        )
                    }
                    val next = state.loggedSets.getOrNull(i + 1)
                    if (next != null) {
                        val restSec = ((next.completedAt - set.completedAt) / 1000L).toInt()
                        RestBetweenSets(restSec)
                    }
                    HorizontalDivider(color = outline.copy(alpha = 0.12f))
                }

                // Live rest timer — sits directly below the last logged set and above the next-set
                // input row (so it always reads "rest, then enter the next set"), the instant a set is
                // logged. Driven off restTimerState (top-level DayUiState.restTimer).
                if (restTimerState != null) {
                    Spacer(Modifier.height(10.dp))
                    InlineRestTimer(timer = restTimerState, onTap = onOpenRestTimerSetter, onSkip = onSkipRest)
                    Spacer(Modifier.height(4.dp))
                }

                // Input row for the next set
                if (!state.skipped) {
                    val targetsMet = state.loggedSets.size >= state.targetSets
                    // This session's last logged set powers long-press-to-repeat on LOG SET
                    // (re-logs its weight + reps).
                    val lastLogged = state.loggedSets.lastOrNull()
                    Spacer(Modifier.height(8.dp))
                    SetInputRow(
                        prefillWeight = state.prefillWeight,
                        suggestedWeight = state.suggestedWeight,
                        suggestionReason = state.suggestionReason,
                        // The PR hint needs the all-time record, not last session — feed the frontier.
                        priorSets = state.priorFrontier,
                        nextSetNumber = state.loggedSets.size + 1,
                        priorSetForActiveRow = state.priorSets.getOrNull(state.loggedSets.size),
                        targetsMet = targetsMet,
                        advanceLabel = advanceLabel,
                        isBodyweight = isBodyweight,
                        isPlates = isPlates,
                        targetReps = targetRepsOf(state.plan.reps),
                        repsPlaceholder = recommendedRepsOf(state.plan.reps),
                        onAdvance = onAdvance,
                        onSubmit = onLogSet,
                        onAddSet = onAddSet,
                        onRepeatLastSet = lastLogged?.let { last -> { onLogSameAsLast(last.id) } }
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

