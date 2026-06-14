package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.parseToLb
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.theme.LocalForgeSettings

// Match SetRow column widths so the input row aligns with logged-set rows.
private val SET_COL_W = 36.dp
private val REPS_COL_W = 48.dp
private val DELTA_COL_W = 72.dp

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
    /** Bodyweight exercise (push-ups, planks…) — no weight field; logs reps only as "BW". */
    isBodyweight: Boolean = false,
    /** Plate-loaded machine/cable exercise — the weight field is a plate COUNT, labelled "PLATES". */
    isPlates: Boolean = false,
    /** Recommended reps from the plan (e.g. 12 from "8-12") — pre-filled into the reps field. */
    targetReps: Int? = null,
    /** Greyed hint shown in the empty reps field — the recommended rep even for AMRAP (e.g. 12). */
    repsPlaceholder: Int? = null,
    onAdvance: () -> Unit = {},
    onSubmit: (weightText: String, reps: Int) -> Unit,
    onAddSet: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    val useKg = LocalForgeSettings.current.useKg
    val plateLb = LocalForgeSettings.current.plateWeightLb
    // Prefill weight + reps with what you did on THIS set last time (#8) — your previous numbers,
    // not the plan target. Plate exercises seed a plate count. First time on the exercise → empty
    // weight + the plan's target reps. Re-seeds per set (keyed on the set number + the seed value).
    val seedWeight = priorSetForActiveRow?.let { p ->
        p.weightLb?.let { lb -> if (isPlates) formatPlateCount(lb / plateLb) else weightInputValue(lb, useKg) } ?: p.weightText
    } ?: prefillWeight.orEmpty()
    val seedReps = priorSetForActiveRow?.reps?.toString() ?: targetReps?.toString().orEmpty()
    // Re-seed only when the SET NUMBER changes (a new set), not when the derived seed value shifts —
    // keying on the volatile seed would wipe a half-typed entry if the prior baseline changed (#11).
    var weight by rememberSaveable(nextSetNumber) { mutableStateOf(seedWeight) }
    var reps by rememberSaveable(nextSetNumber) { mutableStateOf(seedReps) }
    val repsFocus = remember { FocusRequester() }

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
    fun submitSet() {
        val r = reps.toIntOrNull()?.takeIf { it > 0 } ?: return
        if (isBodyweight) {
            onSubmit("BW", r)
        } else {
            if (weight.isBlank()) return
            onSubmit(weight.trim(), r)
        }
        // Fields re-seed from the next set's prior automatically (keyed on the set number) (#8).
    }

    val canSubmit = remember(weight, reps, isBodyweight) {
        reps.toIntOrNull()?.let { it > 0 } == true && (isBodyweight || weight.isNotBlank())
    }

    val prRepsHint = remember(weight, priorSets, useKg) {
        // The field holds a value in the display unit; convert to lb for the PR comparison.
        val weightLb = parseToLb(weight, useKg) ?: return@remember null
        repsNeededForPr(priorSets, weightLb)
    }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val bg = MaterialTheme.colorScheme.background

    if (nextSetNumber != null) {
        // ── Ledger-style table input row ─────────────────────────────────────
        val ctaShape = RoundedCornerShape(16.dp)
        Column(modifier = modifier.fillMaxWidth()) {
            // Active input row — hidden once the target sets are met (then the CTA
            // becomes "MOVE TO NEXT"; tap "+ ADD A SET" to log a bonus set).
            if (!targetsMet) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(onBg.copy(alpha = 0.05f), RoundedCornerShape(10.dp))
                        .padding(horizontal = 12.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.Bottom
                ) {
                    // Set number
                    Box(modifier = Modifier.width(SET_COL_W).padding(bottom = 4.dp)) {
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
                                if (isPlates) "PLATES" else "WEIGHT${if (useKg) " · KG" else " · LB"}",
                                style = MaterialTheme.typography.labelSmall,
                                color = muted,
                                fontSize = 9.sp
                            )
                            Spacer(Modifier.height(2.dp))
                            UnderlineNumberField(
                                value = weight,
                                onValueChange = ::onWeightChange,
                                placeholder = "0",
                                keyboardType = KeyboardType.Text,
                                imeAction = ImeAction.Next,
                                supportingText = if (isPlates) null else prRepsHint?.let { "$it for PR" }
                            )
                        }
                    }

                    // Reps input
                    Box(modifier = Modifier.width(REPS_COL_W).padding(start = 4.dp)) {
                        Column {
                            Text("REPS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                            Spacer(Modifier.height(2.dp))
                            UnderlineNumberField(
                                value = reps,
                                onValueChange = { new -> if (new.all { it.isDigit() }) reps = new },
                                placeholder = repsPlaceholder?.toString() ?: "0",
                                keyboardType = KeyboardType.Number,
                                imeAction = ImeAction.Done,
                                focusRequester = repsFocus,
                                keyboardActions = KeyboardActions(onDone = { submitSet() })
                            )
                        }
                    }

                    // RPE column placeholder — RPE is set on the row after the set is logged.
                    Box(modifier = Modifier.width(44.dp), contentAlignment = Alignment.BottomCenter) {
                        Text("—", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.3f), fontSize = 11.sp)
                    }

                    // Prior set hint ("try 45 × 10") — tap to autofill the inputs.
                    Box(modifier = Modifier.width(DELTA_COL_W), contentAlignment = Alignment.BottomEnd) {
                        priorSetForActiveRow?.let { prior ->
                            // Plate exercises read as a plate count, never the lb equivalent (#9) — so
                            // tapping autofills the plate COUNT into the plate field, not its poundage.
                            val priorDisplay = prior.weightLb?.let { lb ->
                                if (isPlates) "${formatPlateCount(lb / plateLb)} pl" else formatWeight(lb, useKg)
                            } ?: prior.weightText
                            Text(
                                // The ghost to beat — tap to autofill last time's numbers as your floor.
                                "beat $priorDisplay × ${prior.reps}",
                                style = MaterialTheme.typography.labelSmall,
                                color = onBg.copy(alpha = 0.7f),
                                fontSize = 9.sp,
                                textAlign = TextAlign.End,
                                modifier = Modifier.clickable {
                                    weight = prior.weightLb?.let { lb ->
                                        if (isPlates) formatPlateCount(lb / plateLb) else weightInputValue(lb, useKg)
                                    } ?: prior.weightText
                                    reps = prior.reps.toString()
                                }
                            )
                        }
                    }
                }

                Spacer(Modifier.height(12.dp))
            }

            // "+ ADD A SET" — outlined rounded button; extends the planned set count.
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, muted.copy(alpha = 0.6f), ctaShape)
                    .then(if (onAddSet != null) Modifier.clickable { onAddSet() } else Modifier)
                    .padding(vertical = 14.dp),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    "+ ADD A SET",
                    style = MaterialTheme.typography.labelMedium,
                    color = muted
                )
            }

            Spacer(Modifier.height(10.dp))

            if (targetsMet) {
                // Solid white advance CTA — input is done; move to the next exercise.
                Button(
                    onClick = onAdvance,
                    modifier = Modifier.fillMaxWidth(),
                    shape = ctaShape,
                    contentPadding = PaddingValues(vertical = 16.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = Color.White,
                        contentColor = Color.Black
                    )
                ) {
                    Text(advanceLabel, style = MaterialTheme.typography.labelLarge)
                }
            } else {
                // Tap the weight/reps fields to use the system keyboard; this logs the set.
                Button(
                    onClick = { submitSet() },
                    enabled = canSubmit,
                    modifier = Modifier.fillMaxWidth(),
                    shape = ctaShape,
                    contentPadding = PaddingValues(vertical = 16.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = Color.White,
                        contentColor = Color.Black,
                        disabledContainerColor = Color.White.copy(alpha = 0.3f),
                        disabledContentColor = Color.Black.copy(alpha = 0.5f)
                    )
                ) {
                    Text("LOG SET $nextSetNumber", style = MaterialTheme.typography.labelLarge)
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
                    UnderlineNumberField(weight, ::onWeightChange, "0", KeyboardType.Text, ImeAction.Next, supportingText = prRepsHint?.let { "$it for PR" })
                }
                Column(modifier = Modifier.weight(0.7f).padding(start = 16.dp)) {
                    Text("REPS", style = MaterialTheme.typography.labelSmall, color = muted)
                    UnderlineNumberField(reps, { new -> if (new.all { it.isDigit() }) reps = new }, "0", KeyboardType.Number, ImeAction.Done)
                }
                Button(
                    onClick = {
                        val r = reps.toIntOrNull() ?: return@Button
                        onSubmit(weight.trim(), r)
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

@Composable
private fun UnderlineNumberField(
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
    val outline = MaterialTheme.colorScheme.outline

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
        HorizontalDivider(modifier = Modifier.padding(top = 2.dp), thickness = 1.dp, color = outline.copy(alpha = 0.5f))
        if (supportingText != null) {
            Text(supportingText, style = MaterialTheme.typography.labelSmall, color = accent, modifier = Modifier.padding(top = 2.dp))
        }
    }
}

private fun repsNeededForPr(history: List<LoggedSet>, weightLb: Double): Int? {
    val maxRepsAtOrAbove = history
        .filter { it.weightLb != null && it.weightLb >= weightLb }
        .maxOfOrNull { it.reps }
    return maxRepsAtOrAbove?.let { it + 1 }
}
