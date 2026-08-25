@file:OptIn(
    androidx.compose.foundation.ExperimentalFoundationApi::class,
    androidx.compose.foundation.layout.ExperimentalLayoutApi::class
)

package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import kotlinx.coroutines.delay
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import com.forge.app.ui.common.clickableLabeled
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.util.Locale
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.pr.PrDetector
import com.forge.app.domain.units.MAX_REPS_DIGITS
import com.forge.app.domain.units.MAX_HOLD_SECONDS
import com.forge.app.domain.units.WeightUnit
import com.forge.app.service.wear.toProtocol
import com.forge.app.domain.units.formatHold
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.parseToLb
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.theme.LocalForgeSettings

// Hoisted so it isn't recompiled on every weight keystroke. Matches a full "WxR"
// entry (e.g. a pasted "45x10") so it splits into both fields at once.
private val WEIGHT_REPS_REGEX = Regex("""^([0-9]*\.?[0-9]+)\s*[xX]\s*([0-9]+)$""")

/**
 * Input row for the next set. When [nextSetNumber] is provided the layout
 * matches the set-table columns and the LOG SET button is rendered full-width below.
 * Weight/reps use the system keyboard (tap a field); type "BW" in weight for bodyweight.
 */
@Composable
fun SetInputRow(
    prefillWeight: String?,
    suggestedWeight: String? = null,
    suggestionReason: String? = null,
    priorSets: List<LoggedSet> = emptyList(),
    nextSetNumber: Int? = null,
    priorSetForActiveRow: LoggedSet? = null,
    targetsMet: Boolean = false,
    advanceLabel: String = "",
    /** Full-width outlined button shown above LOG SET when [onFinishEarly] is set — ends the
     *  exercise with the sets already logged (AMRAP / gassed out) instead of forcing SKIP.
     *  Rendered uppercased to match the screen's button voice. */
    finishEarlyLabel: String = "",
    /** Null hides the finish-early button (zero sets logged, or targets already met). */
    onFinishEarly: (() -> Unit)? = null,
    /** Bodyweight exercise (push-ups, planks…) — no weight field; logs reps only as "BW". */
    isBodyweight: Boolean = false,
    /** Plate-loaded machine/cable exercise — the weight field is a plate COUNT, labelled "PLATES". */
    isPlates: Boolean = false,
    /** Timed-hold exercise (GYMAP-51) — the reps/RPE/Δ columns become a HOLD readout + stopwatch, and
     *  the set logs a duration (seconds) instead of reps. Every non-timed exercise is unchanged. */
    isTimed: Boolean = false,
    /** Recommended reps from the plan (e.g. 12 from "8-12") — pre-filled into the reps field. */
    targetReps: Int? = null,
    /** Greyed hint shown in the empty reps field — the recommended rep even for AMRAP (e.g. 12). */
    repsPlaceholder: Int? = null,
    onAdvance: () -> Unit = {},
    /** [durationSeconds] non-null = a timed-hold set (GYMAP-51); reps is then 0. */
    onSubmit: (weightText: String, reps: Int, durationSeconds: Int?) -> Unit,
    onAddSet: (() -> Unit)? = null,
    /** Logs a duplicate of this session's last set immediately (long-press on LOG SET). Null until
     *  at least one set is logged this session; null disables the gesture. */
    onRepeatLastSet: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    val plateLb = LocalForgeSettings.current.plateWeightLb
    // Prefill weight + reps with what you did on THIS set last time (#8) — your previous numbers,
    // not the plan target. Plate exercises seed a plate count. First time on the exercise → empty
    // weight + the plan's target reps. Re-seeds per set (keyed on the set number + the seed value).
    val seedWeight = priorSetForActiveRow?.let { p ->
        p.weightLb?.let { lb -> if (isPlates) formatPlateCount(lb / plateLb) else weightInputValue(lb, weightUnit) } ?: p.weightText
    } ?: prefillWeight.orEmpty()
    val seedReps = priorSetForActiveRow?.reps?.toString() ?: targetReps?.toString().orEmpty()
    // Re-seed only when the SET NUMBER changes (a new set), not when the derived seed value shifts —
    // keying on the volatile seed would wipe a half-typed entry if the prior baseline changed (#11).
    var weight by rememberSaveable(nextSetNumber) { mutableStateOf(seedWeight) }
    var reps by rememberSaveable(nextSetNumber) { mutableStateOf(seedReps) }
    val repsFocus = remember { FocusRequester() }
    val haptic = LocalHapticFeedback.current

    // ── Timed-hold state (GYMAP-51) — only exercised when isTimed ────────────────
    // durationSec is the held time in seconds; it's driven by a clock-anchored count-up stopwatch
    // (so it stays accurate across a backgrounded app, like RestTimerController) and by a ±5s
    // manual stepper. Everything re-seeds to zero on a new set (keyed on nextSetNumber).
    var durationSec by rememberSaveable(nextSetNumber) { mutableStateOf(0) }
    var swRunning by rememberSaveable(nextSetNumber) { mutableStateOf(false) }
    var swAnchorMs by rememberSaveable(nextSetNumber) { mutableStateOf(0L) }
    var swBaseSec by rememberSaveable(nextSetNumber) { mutableStateOf(0) }
    // These are rememberSaveable, so they survive PROCESS DEATH — and nothing bounded the gap
    // between the anchor and the resumed read except the one-hour ceiling. Start a plank, take a
    // call, let Android kill the app, reopen two hours later on the same set: the stopwatch
    // restored as RUNNING, elapsed clamped to 3600, and the field read 60:00. One tap on LOG SET
    // wrote a 3600-second hold, permanently, as the all-time best for that exercise. An implausible
    // gap means the user is no longer mid-hold, so stop the clock and keep only what was really
    // held before the app went away.
    LaunchedEffect(Unit) {
        if (swRunning && System.currentTimeMillis() - swAnchorMs > MAX_UNATTENDED_HOLD_MS) {
            swRunning = false
            durationSec = swBaseSec.coerceIn(0, MAX_HOLD_SECONDS)
        }
    }
    LaunchedEffect(swRunning) {
        while (swRunning) {
            val elapsed = ((System.currentTimeMillis() - swAnchorMs) / 1000L).toInt()
            durationSec = (swBaseSec + elapsed).coerceIn(0, MAX_HOLD_SECONDS)
            delay(200)
        }
    }
    fun toggleStopwatch() {
        if (swRunning) {
            swRunning = false
        } else {
            swBaseSec = durationSec
            swAnchorMs = System.currentTimeMillis()
            swRunning = true
        }
        haptic.performHapticFeedback(HapticFeedbackType.LongPress)
    }
    fun stepHold(delta: Int) {
        swRunning = false // a manual nudge stops the running clock so the two inputs never fight
        durationSec = (durationSec + delta).coerceIn(0, MAX_HOLD_SECONDS)
    }

    fun onWeightChange(new: String) {
        // Typing "x"/"X" after a number (e.g. "45x") commits the weight and jumps to the
        // reps field, so the rest of "45x10" is typed as reps — never truncated mid-entry.
        if (new.endsWith("x") || new.endsWith("X")) {
            val numPart = new.dropLast(1)
            if (numPart.isNotEmpty() && numPart.toDoubleOrNull() != null) {
                weight = numPart
                repsFocus.requestFocus()
                return
            }
        }
        // A complete "WxR" (e.g. pasted) splits into both fields at once.
        val match = WEIGHT_REPS_REGEX.matchEntire(new.trim())
        if (match != null) { weight = match.groupValues[1]; reps = match.groupValues[2] }
        else weight = new
    }

    // Single log path used by the LOG SET button and the reps field's "done" action. Validates the
    // weight too (the keyboard's Done bypassed the disabled-button guard, logging a weightless set).
    // Bodyweight exercises log "BW" and skip the weight requirement.
    /**
     * The weight to log: the user's text, unless they never touched the seeded value — in which case
     * hand back the prior set's FULL-PRECISION display weight instead of the rounded seed.
     *
     * `weightInputValue` renders one decimal, which in STONES is a granularity of 0.1 st = 1.4 lb.
     * "Repeat last set" on a 135 lb lift seeded "9.6" and logged 9.6 × 14 = 134.4 lb — 0.6 lb light,
     * past the 0.5 lb epsilon, so the row painted a phantom "−0.6 lb" drop for a set the user had
     * deliberately not changed. [SetRow]'s edit path has carried this guard for a while; the log
     * path did not. LB and plate counts need no conversion, so they pass straight through.
     */
    fun untouchedSeedOrTyped(): String {
        val typed = weight.trim()
        val priorLb = priorSetForActiveRow?.weightLb
        return if (!isPlates && weightUnit != WeightUnit.LB && priorLb != null && typed == seedWeight)
            toDisplayWeight(priorLb, weightUnit).toString()
        else typed
    }

    fun submitSet() {
        if (isTimed) {
            val d = durationSec.takeIf { it > 0 } ?: return
            swRunning = false
            // Weight is optional on a hold (weighted plank/hang); bodyweight or blank logs "BW".
            val wt = if (isBodyweight || weight.isBlank()) "BW" else untouchedSeedOrTyped()
            onSubmit(wt, 0, d)
            return
        }
        val r = reps.toIntOrNull()?.takeIf { it > 0 } ?: return
        if (isBodyweight) {
            onSubmit("BW", r, null)
        } else {
            if (weight.isBlank()) return
            onSubmit(untouchedSeedOrTyped(), r, null)
        }
        // Fields re-seed from the next set's prior automatically (keyed on the set number) (#8).
    }

    // ── +/- steppers — nudge the field without opening the keyboard ──────────────
    // Weight steps in the DISPLAY unit (kg/lb) or by half-plates on PLATES exercises; the field
    // already holds a display-unit value, so we step the parsed number and re-format it.
    fun stepWeight(delta: Double) {
        val base = weight.toDoubleOrNull() ?: 0.0
        val next = (base + delta).coerceAtLeast(0.0)
        // Format/parse pinned to Locale.US so the field always uses a '.' decimal — a comma-decimal
        // locale would otherwise write "2,5" and the next toDoubleOrNull() would fail (matches WeightFormatter).
        weight = if (next % 1.0 == 0.0) next.toInt().toString() else String.format(Locale.US, "%.1f", next)
    }
    fun stepReps(delta: Int) {
        val base = reps.toIntOrNull() ?: targetReps ?: repsPlaceholder ?: 0
        reps = (base + delta).coerceAtLeast(0).toString()
    }

    val canSubmit = remember(weight, reps, isBodyweight, isTimed, durationSec) {
        if (isTimed) durationSec > 0
        else reps.toIntOrNull()?.let { it > 0 } == true && (isBodyweight || weight.isNotBlank())
    }

    val prRepsHint = remember(weight, priorSets, weightUnit) {
        // The field holds a value in the display unit; convert to lb for the PR comparison.
        val weightLb = parseToLb(weight, weightUnit) ?: return@remember null
        repsNeededForPr(priorSets, weightLb)
    }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val bg = MaterialTheme.colorScheme.background
    val outline = MaterialTheme.colorScheme.outline

    if (nextSetNumber != null) {
        // ── Ledger-style table input row ─────────────────────────────────────
        // §8 — every do-it-now action in the app is a CAPSULE (ForgePrimaryCapsule /
        // ForgeOutlineCapsule / the top bar's FINISH). This slot used to be a 16dp rounded
        // rectangle, which is M3's stock button corner and read as another app's button
        // sitting inside ours.
        val ctaShape = RoundedCornerShape(50)
        val stacked = SetTable.stacked()
        Column(modifier = modifier.fillMaxWidth()) {
            // Active input row — hidden once the target sets are met (then the CTA
            // becomes "MOVE TO NEXT"; tap "+ ADD A SET" to log a bonus set).
            // Timed holds take the parallel HOLD/stopwatch body below; every other exercise is unchanged.
            if (!targetsMet && !isTimed) {
                val repsField: @Composable (Modifier) -> Unit = { m ->
                // Reps input
                Box(modifier = m.padding(start = 4.dp)) {
                    Column {
                        Text("REPS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                        Spacer(Modifier.height(2.dp))
                        BigNumberField(
                            value = reps,
                            onValueChange = { new -> if (new.all { it.isDigit() }) reps = new.take(MAX_REPS_DIGITS) },
                            placeholder = repsPlaceholder?.toString() ?: "0",
                            keyboardType = KeyboardType.Number,
                            imeAction = ImeAction.Done,
                            focusRequester = repsFocus,
                            keyboardActions = KeyboardActions(onDone = { submitSet() })
                        )
                    }
                }
                }
                val ghostHint: @Composable (Modifier) -> Unit = { m ->
                // Prior set hint ("try 45 × 10") — tap to autofill the inputs.
                Box(modifier = m, contentAlignment = Alignment.BottomEnd) {
                    priorSetForActiveRow?.let { prior ->
                        // Plate exercises read as a plate count, never the lb equivalent (#9) — so
                        // tapping autofills the plate COUNT into the plate field, not its poundage.
                        val priorDisplay = prior.weightLb?.let { lb ->
                            if (isPlates) "${formatPlateCount(lb / plateLb)} pl" else formatWeight(lb, weightUnit)
                        } ?: prior.weightText
                        Text(
                            // The ghost to beat — tap to autofill the weight only so the user
                            // pushes for one extra rep rather than copying the exact same set.
                            "beat $priorDisplay × ${prior.reps}",
                            style = MaterialTheme.typography.labelSmall,
                            color = onBg.copy(alpha = 0.7f),
                            fontSize = 9.sp,
                            textAlign = TextAlign.End,
                            modifier = Modifier
                                .sizeIn(minWidth = 48.dp, minHeight = 48.dp)
                                .clickableLabeled(
                                    "Autofill last session's weight"
                                ) {
                                    weight = prior.weightLb?.let { lb ->
                                        if (isPlates) formatPlateCount(lb / plateLb) else weightInputValue(lb, weightUnit)
                                    } ?: prior.weightText
                                    // Reps intentionally left for the user — aim for one more.
                                }
                        )
                    }
                }
                }
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
                        .padding(horizontal = 12.dp, vertical = 10.dp)
                ) {
                Row(verticalAlignment = Alignment.Bottom) {
                    // Set number
                    Box(modifier = Modifier.width(SetTable.SET_COL_W).padding(bottom = 4.dp)) {
                        Text(
                            "%02d".format(nextSetNumber),
                            style = MaterialTheme.typography.labelSmall,
                            color = muted,
                            fontSize = 9.sp
                        )
                    }

                    // Weight input — tapping the "· LB/KG" label offers a unit switch. Bodyweight
                    // exercises have no weight to enter, so we show a static "BW" instead of a field.
                    if (isBodyweight) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text("BODYWEIGHT", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                            Spacer(Modifier.height(2.dp))
                            Text("BW", style = MaterialTheme.typography.headlineMedium, color = onBg)
                        }
                    } else {
                        Column(modifier = Modifier.weight(1f)) {
                            // Plate exercises show "PLATES" (the value is a plate count); free weights
                            // show "WEIGHT · LB/KG". (Unit is changed in Settings, not by tapping here — #5.)
                            Text(
                                if (isPlates) "PLATES" else "WEIGHT · ${unitLabel(weightUnit).uppercase()}",
                                style = MaterialTheme.typography.labelSmall,
                                color = muted,
                                fontSize = 9.sp
                            )
                            Spacer(Modifier.height(2.dp))
                            BigNumberField(
                                value = weight,
                                onValueChange = ::onWeightChange,
                                placeholder = "0",
                                keyboardType = KeyboardType.Text,
                                imeAction = ImeAction.Next,
                                supportingText = if (isPlates) null else prRepsHint?.let { "$it for PR" }
                            )
                        }
                    }

                    if (!stacked) {
                        repsField(Modifier.width(SetTable.REPS_COL_W).padding(start = 4.dp))
                        Box(modifier = Modifier.width(SetTable.RPE_COL_W), contentAlignment = Alignment.BottomCenter) {
                            Text("—", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.3f), fontSize = 11.sp)
                        }
                        ghostHint(Modifier.width(SetTable.DELTA_COL_W))
                    }
                }
                if (stacked) {
                    // Reps and the ghost drop below the weight field rather than sharing its line:
                    // a headlineMedium field cannot live in a 48dp column at 2x (§14, see SetTable).
                    // The RPE placeholder is gone with the column it was holding open.
                    Row(
                        modifier = Modifier.padding(start = SetTable.SET_COL_W, top = 12.dp),
                        verticalAlignment = Alignment.Bottom
                    ) {
                        repsField(Modifier.weight(1f))
                        ghostHint(Modifier)
                    }
                }
                }

                // ── Quick-adjust row — +/- steppers + inline "+ SET" (repeat-last-set is
                // hold-LOG-SET). "Add a set" rides here as a compact pill so the prominent
                // full-width slot below can carry the exercise-finish action instead.
                Spacer(Modifier.height(10.dp))
                FlowRow(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    // The step table lives in :shared (W1) so the phone's ± and the watch's bezel
                    // adjust can never drift; the unit mapping is the wear publisher's.
                    val weightStep = com.forge.shared.weight.WeightSteps.weightStep(weightUnit.toProtocol(), isPlates)
                    if (!isBodyweight) {
                        StepperPill(
                            label = if (isPlates) "PL" else unitLabel(weightUnit).uppercase(),
                            onMinus = { stepWeight(-weightStep) },
                            onPlus = { stepWeight(weightStep) }
                        )
                    }
                    StepperPill(label = "REPS", onMinus = { stepReps(-1) }, onPlus = { stepReps(1) })
                    // Extends the planned set count; sized/shaped to match the stepper pills.
                    AddSetPill(onAdd = onAddSet)
                }

                Spacer(Modifier.height(12.dp))
            }

            // ── Timed-hold input (GYMAP-51) — HOLD readout + stopwatch, replacing weight/reps ──
            if (!targetsMet && isTimed) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
                        .padding(horizontal = 12.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.Bottom
                ) {
                    Box(modifier = Modifier.width(SetTable.SET_COL_W).padding(bottom = 4.dp)) {
                        Text(
                            "%02d".format(nextSetNumber),
                            style = MaterialTheme.typography.labelSmall,
                            color = muted,
                            fontSize = 9.sp
                        )
                    }
                    // HOLD readout — the big mm:ss, accent while the stopwatch is running.
                    Column(modifier = Modifier.weight(1f)) {
                        Text("HOLD", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                        Spacer(Modifier.height(2.dp))
                        Text(
                            formatHold(durationSec),
                            style = MaterialTheme.typography.headlineMedium,
                            color = if (swRunning) MaterialTheme.colorScheme.primary else onBg
                        )
                    }
                    // The hold to beat — tap to autofill last session's time.
                    Box(modifier = Modifier.width(SetTable.DELTA_COL_W), contentAlignment = Alignment.BottomEnd) {
                        priorSetForActiveRow?.durationSeconds?.let { priorSec ->
                            Text(
                                "beat ${formatHold(priorSec)}",
                                style = MaterialTheme.typography.labelSmall,
                                color = onBg.copy(alpha = 0.7f),
                                fontSize = 9.sp,
                                textAlign = TextAlign.End,
                                modifier = Modifier
                                    .sizeIn(minWidth = 48.dp, minHeight = 48.dp)
                                    .clickableLabeled("Autofill last session's hold time") {
                                        swRunning = false
                                        durationSec = priorSec.coerceIn(0, MAX_HOLD_SECONDS)
                                    }
                            )
                        }
                    }
                }

                // Stopwatch + ±5s adjust — the timed analogue of the weight/reps steppers,
                // carrying the same inline "+ SET" pill so timed holds can add a set too.
                Spacer(Modifier.height(10.dp))
                FlowRow(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    StopwatchButton(running = swRunning, onToggle = { toggleStopwatch() })
                    StepperPill(label = "5 SEC", onMinus = { stepHold(-5) }, onPlus = { stepHold(5) })
                    AddSetPill(onAdd = onAddSet)
                }

                Spacer(Modifier.height(12.dp))
            }

            if (targetsMet) {
                // Targets hit — the input row (with its inline "+ SET") is gone, so
                // "add a bonus set" takes the full-width slot, then the filled advance
                // CTA moves to the next exercise.
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .border(1.dp, outline.copy(alpha = 0.35f), ctaShape)
                        .then(if (onAddSet != null) Modifier.clickable { onAddSet() } else Modifier)
                        .padding(vertical = 14.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text("+ ADD A SET", style = MaterialTheme.typography.labelMedium, color = muted)
                }
                Spacer(Modifier.height(10.dp))
                FilledCta(
                    label = advanceLabel,
                    shape = ctaShape,
                    onClickLabel = advanceLabel,
                    onClick = onAdvance
                )
            } else {
                // "Done with this exercise" — end the exercise with what you logged (files it
                // under DONE, not skipped) and move on. It's the sidekick to LOG SET, so it's a
                // full-width OUTLINED button in the prominent slot (§8 ②) — not a stretched text
                // link. Sits where "+ ADD A SET" used to. Shown only once ≥1 set is logged.
                if (onFinishEarly != null) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .border(1.dp, outline.copy(alpha = 0.35f), ctaShape)
                            .clickableLabeled(finishEarlyLabel) { onFinishEarly() }
                            .padding(vertical = 14.dp),
                        contentAlignment = Alignment.Center
                    ) {
                        Text(
                            finishEarlyLabel.uppercase(),
                            style = MaterialTheme.typography.labelMedium,
                            color = onBg
                        )
                    }
                    Spacer(Modifier.height(10.dp))
                }

                // Tap the weight/reps fields to use the system keyboard; tapping this logs the set.
                // A Material Button can't take a long-press, so this is a styled Box with
                // combinedClickable: tap = log the typed set, long-press = repeat your last set.
                val canRepeat = onRepeatLastSet != null
                // Three visual states: ready-to-log (solid), hold-to-repeat-only (mid, clearly still
                // interactive — not the fully-dimmed look that reads as "disabled"), and inert.
                // combinedClickable(enabled = …) carries the disabled state to TalkBack for the inert case.
                val bgAlpha = when { canSubmit -> 1f; canRepeat -> 0.55f; else -> 0.3f }
                val fgAlpha = when { canSubmit -> 1f; canRepeat -> 0.75f; else -> 0.5f }
                FilledCta(
                    label = "LOG SET $nextSetNumber",
                    shape = ctaShape,
                    enabled = canSubmit || canRepeat,
                    bgAlpha = bgAlpha,
                    fgAlpha = fgAlpha,
                    onClickLabel = if (canSubmit) "Log set $nextSetNumber" else null,
                    onLongClickLabel = if (canRepeat) "Repeat last set" else null,
                    onLongClick = if (canRepeat) {
                        {
                            haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                            onRepeatLastSet?.invoke()
                        }
                    } else null,
                    onClick = { if (canSubmit) submitSet() }
                )
                if (canRepeat) {
                    Spacer(Modifier.height(6.dp))
                    Text(
                        "Hold to repeat your last set",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted.copy(alpha = 0.7f),
                        fontSize = 10.sp,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth()
                    )
                }
            }
        }
    } else {
        // ── Legacy compact layout (kept for any other call sites) ────────────
        Column(modifier = modifier.fillMaxWidth()) {
            if (suggestedWeight != null) {
                Row(
                    modifier = Modifier.fillMaxWidth().padding(bottom = 2.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    val hint = buildString {
                        // PLATES suggestions are already self-describing ("3 plates").
                        append(if (isPlates) "Try: $suggestedWeight" else "Try: $suggestedWeight lb")
                        if (!suggestionReason.isNullOrBlank()) append(" · $suggestionReason")
                    }
                    Text(hint, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary.copy(alpha = 0.75f), modifier = Modifier.weight(1f))
                }
            }
            Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
                Column(modifier = Modifier.weight(1f)) {
                    Text("WEIGHT · LB", style = MaterialTheme.typography.labelSmall, color = muted)
                    BigNumberField(weight, ::onWeightChange, "0", KeyboardType.Text, ImeAction.Next, supportingText = prRepsHint?.let { "$it for PR" })
                }
                Column(modifier = Modifier.weight(0.7f).padding(start = 16.dp)) {
                    Text("REPS", style = MaterialTheme.typography.labelSmall, color = muted)
                    BigNumberField(reps, { new -> if (new.all { it.isDigit() }) reps = new.take(MAX_REPS_DIGITS) }, "0", KeyboardType.Number, ImeAction.Done)
                }
                Button(
                    onClick = {
                        val r = reps.toIntOrNull() ?: return@Button
                        onSubmit(weight.trim(), r, null)
                        reps = ""
                    },
                    enabled = canSubmit,
                    shape = RoundedCornerShape(50),
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = onBg,
                        contentColor = bg,
                        disabledContainerColor = onBg.copy(alpha = 0.35f),
                        disabledContentColor = bg.copy(alpha = 0.7f)
                    )
                ) { Text("Log set →", style = MaterialTheme.typography.labelSmall) }
            }
            if (prefillWeight != null && weight.isBlank()) {
                Text(
                    "Use last: $prefillWeight",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.primary,
                    fontStyle = FontStyle.Italic,
                    modifier = Modifier.padding(top = 2.dp)
                )
            }
        }
    }
}

/**
 * The one filled action in the input row's prominent slot — LOG SET while sets remain, then
 * MOVE TO NEXT / FINISH WORKOUT once the targets are met. They are the same button in two states, so
 * they share one definition rather than two that drift.
 *
 * Accent-filled, on Antho's call (2026-08-23): §8 reserves the accent ground for a hub tab's one
 * primary action, but the session screen is where the app is actually used and it was spending no
 * accent at all, so the rule now reads "the screen's one do-it-now action" here too. Same treatment
 * as [com.forge.app.ui.common.ForgeHeroAction] — accent ground, `onPrimary` label (which flips to
 * the background tone above luminance 0.18, so a mid-tone warm accent still gets dark text and a
 * monochrome accent still reads), bold mono — so this and Home's CTA are visibly one button.
 *
 * A Box rather than [com.forge.app.ui.common.ForgePrimaryCapsule] because LOG SET carries a
 * long-press (hold to repeat your last set), which a Material Button can't take and the shared
 * capsule has no slot for; [bgAlpha]/[fgAlpha] carry its three states (ready / hold-to-repeat-only /
 * inert).
 */
@Composable
private fun FilledCta(
    label: String,
    shape: Shape,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    bgAlpha: Float = 1f,
    fgAlpha: Float = 1f,
    onClickLabel: String? = null,
    onLongClickLabel: String? = null,
    onLongClick: (() -> Unit)? = null
) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(MaterialTheme.colorScheme.primary.copy(alpha = bgAlpha))
            .combinedClickable(
                enabled = enabled,
                onClickLabel = onClickLabel,
                onLongClickLabel = onLongClickLabel,
                onLongClick = onLongClick,
                onClick = onClick
            )
            .padding(vertical = 16.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.onPrimary.copy(alpha = fgAlpha)
        )
    }
}

/** A compact pill that rides the quick-adjust row and appends one set to the plan.
 *  Sized/shaped to sit flush with the +/- stepper pills beside it. */
@Composable
private fun AddSetPill(onAdd: (() -> Unit)?) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    Box(
        modifier = Modifier
            .sizeIn(minHeight = 40.dp)
            .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(50))
            .then(if (onAdd != null) Modifier.clickableLabeled("Add a set") { onAdd() } else Modifier)
            .padding(horizontal = 18.dp),
        contentAlignment = Alignment.Center
    ) {
        Text("+ SET", style = MaterialTheme.typography.labelMedium, color = muted)
    }
}

/**
 * Start/Stop control for a timed hold (GYMAP-51) — a 44dp capsule in the row's own language: an
 * outline when idle ("Start hold"), an accent wash + accent border + accent label while the count-up
 * runs ("Stop"). The whole capsule is the tap target with a spoken a11y label.
 */
@Composable
private fun StopwatchButton(
    running: Boolean,
    onToggle: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(50))
            .then(if (running) Modifier.background(accent.copy(alpha = 0.15f)) else Modifier)
            .border(1.dp, if (running) accent else outline.copy(alpha = 0.35f), RoundedCornerShape(50))
            .clickableLabeled(if (running) "Stop the hold timer" else "Start the hold timer") { onToggle() }
            .sizeIn(minWidth = 96.dp, minHeight = 44.dp)
            .padding(horizontal = 16.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            if (running) "Stop" else "Start hold",
            style = MaterialTheme.typography.labelLarge,
            color = if (running) accent else onBg
        )
    }
}

/** A compact rounded "−  LABEL  +" control that nudges a field without opening the keyboard.
 *  Each arrow is a ≥44dp touch target with a spoken a11y label. */
@Composable
private fun StepperPill(
    label: String,
    onMinus: () -> Unit,
    onPlus: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(50))
    ) {
        Box(
            modifier = Modifier
                .sizeIn(minWidth = 44.dp, minHeight = 40.dp)
                .clickableLabeled("Decrease $label") { onMinus() },
            contentAlignment = Alignment.Center
        ) { Text("−", style = MaterialTheme.typography.titleMedium, color = onBg) }
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
        Box(
            modifier = Modifier
                .sizeIn(minWidth = 44.dp, minHeight = 40.dp)
                .clickableLabeled("Increase $label") { onPlus() },
            contentAlignment = Alignment.Center
        ) { Text("+", style = MaterialTheme.typography.titleMedium, color = onBg) }
    }
}

/**
 * The set-input number field. No rule under it: the row's own surface fill and the +/- steppers
 * already say the number is editable, so a line there would be decoration, and a line is a claim
 * about data (DESIGN 1).
 */
@Composable
private fun BigNumberField(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    keyboardType: KeyboardType,
    imeAction: ImeAction,
    modifier: Modifier = Modifier,
    supportingText: String? = null,
    focusRequester: FocusRequester? = null,
    keyboardActions: KeyboardActions = KeyboardActions.Default
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    Column(modifier = modifier) {
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            textStyle = MaterialTheme.typography.headlineMedium.copy(color = onBg),
            singleLine = true,
            cursorBrush = SolidColor(accent),
            keyboardOptions = KeyboardOptions(keyboardType = keyboardType, imeAction = imeAction),
            keyboardActions = keyboardActions,
            modifier = focusRequester?.let { Modifier.focusRequester(it) } ?: Modifier,
            decorationBox = { inner ->
                Box {
                    if (value.isEmpty()) {
                        Text(placeholder, style = MaterialTheme.typography.headlineMedium, color = muted.copy(alpha = 0.4f))
                    }
                    inner()
                }
            }
        )
        if (supportingText != null) {
            Text(supportingText, style = MaterialTheme.typography.labelSmall, color = accent, modifier = Modifier.padding(top = 2.dp))
        }
    }
}

/**
 * The lowest rep count at which this weight would be a record — asked of [PrDetector] itself rather
 * than re-derived.
 *
 * The hint used to run its own rule: it thresholded the wrong dimension and dropped the `isAssisted`
 * filter entirely. A pull-up history of one BAND-ASSISTED 100 lb x 10 made it print "11 for PR" at
 * 100 lb, while `PrDetector` — which ignores assisted sets — would have flagged a record at ONE rep.
 * Two answers to the same question, on the same screen.
 *
 * Null when the weight is never a record here, or when the answer is far enough out that showing it
 * is discouraging rather than useful.
 */
private fun repsNeededForPr(history: List<LoggedSet>, weightLb: Double): Int? {
    for (n in 1..MAX_PR_HINT_REPS) {
        if (PrDetector.isPr(history, weightLb, n)) return n
    }
    return null
}

/** Beyond this the hint stops being a nudge. Also bounds the search for a weight that never wins. */
private const val MAX_PR_HINT_REPS = 50

/** A running hold older than this can't be a hold still in progress — the app was killed or the
 *  clock jumped. Comfortably past any real weighted plank, and well under the 1 h field ceiling. */
private const val MAX_UNATTENDED_HOLD_MS = 15L * 60 * 1000
