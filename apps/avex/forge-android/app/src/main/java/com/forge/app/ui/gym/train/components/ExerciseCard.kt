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
import com.forge.app.domain.volume.volumeLb
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
import com.forge.app.domain.units.unitLabel
import com.forge.app.program.ExerciseUnit
import com.forge.app.ui.theme.LocalForgeSettings
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

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ExerciseCard(
    exerciseIndex: Int,
    state: ExerciseUiState,
    isNow: Boolean = false,
    totalExercises: Int = 0,
    /** One flag per exercise in the day, true once it's done or skipped — drives [SessionRail].
     *  Empty hides the rail. */
    sessionDone: List<Boolean> = emptyList(),
    restTimerState: RestTimerState? = null,
    sessionStartedAtMs: Long? = null,
    onToggle: () -> Unit,
    onLogSet: (weightText: String, reps: Int, durationSeconds: Int?) -> Unit,
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
    /** Secondary CTA to end this exercise before all target sets are logged (marks it done, not
     *  skipped, then advances). Shown only once ≥1 set is logged and targets aren't met. */
    finishEarlyLabel: String = "",
    onFinishEarly: () -> Unit = {},
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

                // Read the EFFECTIVE unit (swap-aware), not the static plan's, so swapping a
                // bodyweight slot for a weighted one (or vice-versa) switches the weight type too.
                val isBodyweight = state.effectiveUnit == ExerciseUnit.BODYWEIGHT
                val isPlates = state.effectiveUnit == ExerciseUnit.PLATES
                // Timed holds (GYMAP-51) log a duration, not reps — the REPS column reads HOLD and the
                // input row swaps in a stopwatch. Resolved swap-aware off the effective exercise.
                val isTimed = state.timed

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
                    if (sessionDone.isNotEmpty()) {
                        // 6 above, 12 below: the rail belongs to the counter (one unit, tight), and
                        // §7 wants real air before a role change — here a 52sp serif hero.
                        Spacer(Modifier.height(6.dp))
                        SessionRail(done = sessionDone, currentIndex = exerciseIndex)
                        Spacer(Modifier.height(8.dp))
                    }
                    Spacer(Modifier.height(4.dp))
                }

                // Exercise name (serif hero) — tap to open the swap picker.
                //
                // The hero steps DOWN as the name gets longer. At a flat 52sp a three-word lift
                // ("Incline Barbell Bench") wrapped to three lines and pushed the set table off the
                // fold, so the name — the one thing you already know — cost the most screen. Short
                // names keep the full 52sp; the scale below stays inside the serif display/headline
                // ramp (§7), so a stepped-down title is still the same voice, just quieter.
                val heroStyle = when {
                    state.effectiveName.length <= 13 -> MaterialTheme.typography.displayLarge   // "Hack Squat"
                    state.effectiveName.length <= 20 -> MaterialTheme.typography.headlineLarge  // "Smith Bench Press"
                    else -> MaterialTheme.typography.headlineMedium                             // "Incline Barbell Bench"
                }
                Text(
                    state.effectiveName,
                    style = heroStyle,
                    color = onBg,
                    textDecoration = if (state.skipped) TextDecoration.LineThrough else TextDecoration.None,
                    modifier = Modifier.clickable { onOpenSwapPicker() }
                )

                Spacer(Modifier.height(6.dp))

                // Plan line — the whole brief for this exercise in ONE line: what to do, what you
                // did last time, and the one number to aim for. It used to be three stacked italic
                // lines (target / "Suggested next → 140 (keep this weight — reach 12 reps on every
                // set before adding load)" / the same again for reps), which buried the set table
                // under a paragraph of coaching nobody re-reads mid-set. The reason text is gone
                // rather than moved: the number IS the advice, and the sentence explaining it only
                // earned its place when it had a line to itself.
                val priorLastSet = state.priorSets.lastOrNull()
                // The cue is a weight on weighted lifts and reps on bodyweight ones (CO5); it keeps
                // the accent, since it is the only part of the line asking you to change what you
                // were about to do. The unit rides along — the field below is labelled in the
                // display unit and a bare "140" invited a kg user to log 140 kg
                // (docs/bug-scan-2026-08/03-units-math.md).
                val accent = MaterialTheme.colorScheme.primary
                val weightUnit = LocalForgeSettings.current.weightUnit
                val cue = when {
                    state.suggestedWeight != null ->
                        if (isPlates) "try ${state.suggestedWeight} pl"
                        else "try ${state.suggestedWeight} ${unitLabel(weightUnit)}"
                    state.suggestedReps != null -> "try ${state.suggestedReps} reps"
                    else -> null
                }
                val planLine = buildAnnotatedString {
                    withStyle(SpanStyle(fontStyle = FontStyle.Italic)) {
                        append("${state.plan.sets} × ${state.plan.reps}")
                        if (priorLastSet != null) {
                            append(" · last ${priorLastSet.weightText} × ${priorLastSet.reps}")
                        } else {
                            append(" · first time")
                        }
                        if (cue != null) {
                            append(" · ")
                            withStyle(SpanStyle(color = accent)) { append(cue) }
                        }
                    }
                }
                Text(planLine, style = MaterialTheme.typography.bodySmall, color = muted)

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
                    state.loggedSets.map { it.volumeLb() }
                }
                val currentVolumeLb = remember(currentVolumes) { currentVolumes.sum() }
                val priorVolumes = remember(state.priorSets) {
                    state.priorSets.map { it.volumeLb() }
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

                // No "N / M SETS · X LB" chip here — the strip above already carries both, and a
                // second copy two lines down was pure restatement.
                Spacer(Modifier.height(16.dp))

                // ── Set table ─────────────────────────────────────────────────
                val stacked = SetTable.stacked()
                // Table header (5 cols: SET | WEIGHT | REPS | RPE | Δ LAST). Above the stacking
                // threshold the rows below are two lines carrying their own inline labels, so the
                // trailing three headers would label nothing — only SET and the weight column
                // still head a column (§14, see SetTable).
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    // The gutter is 36dp because a two-digit set number fits it at any scale, but
                    // the word "SET" does not once it is 18sp — so when stacked the header cell
                    // grows to its text and pushes the weight label along instead of printing
                    // "SETPLATES".
                    Text(
                        "SET",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp,
                        modifier = if (stacked) Modifier.padding(end = 10.dp)
                        else Modifier.width(SetTable.SET_COL_W)
                    )
                    Text(
                        when { isBodyweight -> "BODYWEIGHT"; isPlates -> "PLATES"; else -> "WEIGHT · LB" },
                        style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.weight(1f)
                    )
                    if (!stacked) {
                        Text(if (isTimed) "HOLD" else "REPS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.width(SetTable.REPS_COL_W))
                        Text("RPE", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.width(SetTable.RPE_COL_W), textAlign = TextAlign.Center)
                        Text("LAST", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.width(SetTable.DELTA_COL_W), textAlign = TextAlign.End)
                    }
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
                            isTimed = isTimed,
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
                        // "I'm done" path — only meaningful once you've logged something and haven't hit
                        // the target yet (with zero sets that's a skip; at target the advance CTA shows).
                        finishEarlyLabel = finishEarlyLabel,
                        onFinishEarly = if (!targetsMet && state.loggedSets.isNotEmpty()) onFinishEarly else null,
                        isBodyweight = isBodyweight,
                        isPlates = isPlates,
                        isTimed = isTimed,
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

