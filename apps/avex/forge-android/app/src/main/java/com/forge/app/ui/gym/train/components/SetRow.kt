package com.forge.app.ui.gym.train.components

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.TrendingDown
import androidx.compose.material.icons.filled.TrendingUp
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.formatWeightDelta
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.rirLabel
import com.forge.app.ui.common.rpeLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.ForgePrGold
import com.forge.app.ui.theme.LocalForgeSettings

private val SET_COL_W = 36.dp
private val REPS_COL_W = 48.dp
private val RPE_COL_W = 44.dp
private val DELTA_COL_W = 72.dp

/** Weight deltas at or below this (lb) are treated as "matched" — absorbs kg/plate rounding noise so a
 *  sub-pound float difference doesn't render a phantom signed delta + trend arrow (#11). */
private const val WEIGHT_DELTA_EPS_LB = 0.5

/** Plate count display: whole counts drop the decimal ("3"), halves keep one ("2.5"). */
internal fun formatPlateCount(plates: Double): String =
    if (plates % 1.0 == 0.0) plates.toInt().toString() else "%.1f".format(plates)

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SetRow(
    set: LoggedSet,
    setIndex: Int = 1,
    isPr: Boolean,
    isPlates: Boolean = false,
    priorSet: LoggedSet? = null,
    /** Last session's FINAL set — the fallback baseline for the LAST delta when you did more sets
     *  this session than last time (so the column is never blank on a bonus set). */
    priorFallbackSet: LoggedSet? = null,
    onDelete: () -> Unit,
    onEdit: (weightText: String, reps: Int) -> Unit,
    onLongPress: (() -> Unit)? = null,
    onToggleDifficultyTag: ((String?) -> Unit)? = null,
    onSetRpe: ((Double?) -> Unit)? = null,
    onToggleAmrap: (() -> Unit)? = null,
    onToggleAssisted: (() -> Unit)? = null,
    onToggleFailure: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    val useKg = LocalForgeSettings.current.useKg
    val plateLb = LocalForgeSettings.current.plateWeightLb
    // Plate exercises read as a plate count + the lb equivalent (e.g. "3 plates (45 lb)"); the edit
    // field seeds with the plate count. Free weights show/seed the weight in the display unit.
    val displayWeight = set.weightLb?.let { lb ->
        if (isPlates) "${formatPlateCount(lb / plateLb)} plates (${formatWeight(lb, useKg)})"
        else formatWeight(lb, useKg)
    } ?: set.weightText
    val editSeed = set.weightLb?.let { lb ->
        if (isPlates) formatPlateCount(lb / plateLb) else weightInputValue(lb, useKg)
    } ?: set.weightText
    var isEditing by remember(set.id) { mutableStateOf(false) }
    var editWeight by remember(set.id, useKg, set.weightText) { mutableStateOf(editSeed) }
    var editReps by remember(set.id, set.reps) { mutableStateOf(set.reps.toString()) }
    var showRpePicker by remember { mutableStateOf(false) }

    // The visible "set logged" beat: rise + fade + a scale that lands with a spring so the
    // row "thunks" into place (matching the haptic). A PR row gets a bouncier overshoot so
    // it lands harder than a normal set.
    val appear = remember { Animatable(0f) }
    LaunchedEffect(Unit) {
        appear.animateTo(1f, animationSpec = if (isPr) ForgeMotion.bouncy() else ForgeMotion.snappy())
    }

    val accent = MaterialTheme.colorScheme.primary
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline

    // LAST delta vs the prior set, in the exercise's own unit (plate count on PLATES, display unit
    // otherwise) and ALWAYS signed — "+0" when you match, negative when lower (#2/#6). Compares to
    // last session's set at this position; if you did more sets than last time it falls back to last
    // session's final set so the column is never blank. Bodyweight (null weightLb) uses the rep delta.
    val baseline = priorSet ?: priorFallbackSet
    val curLb = set.weightLb
    val baseLb = baseline?.weightLb
    val wDiff = if (curLb != null && baseLb != null) curLb - baseLb else null
    val rDiff = if (baseline != null) set.reps - baseline.reps else 0
    val deltaPositive = (wDiff != null && wDiff > WEIGHT_DELTA_EPS_LB) ||
        ((wDiff == null || kotlin.math.abs(wDiff) <= WEIGHT_DELTA_EPS_LB) && rDiff > 0)
    val deltaNegative = (wDiff != null && wDiff < -WEIGHT_DELTA_EPS_LB) ||
        ((wDiff == null || kotlin.math.abs(wDiff) <= WEIGHT_DELTA_EPS_LB) && rDiff < 0)
    val deltaLabel: String? = when {
        baseline == null -> null
        wDiff != null && kotlin.math.abs(wDiff) > WEIGHT_DELTA_EPS_LB ->
            if (isPlates) {
                val pd = wDiff / plateLb
                (if (pd >= 0) "+" else "") + formatPlateCount(pd) + " pl"
            } else (if (wDiff >= 0) "+" else "") + formatWeightDelta(wDiff, useKg)
        rDiff != 0 -> (if (rDiff > 0) "+" else "") + "$rDiff rep"
        // Matched exactly — show a deliberate zero rather than nothing (#6).
        wDiff != null -> if (isPlates) "+0 pl" else "+0 ${unitLabel(useKg)}"
        else -> "+0 rep"
    }
    val deltaColor = when {
        deltaPositive -> ForgeLastGreen
        deltaNegative -> muted.copy(alpha = 0.7f)
        else -> muted.copy(alpha = 0.5f)
    }

    if (isEditing) {
        val canConfirm = editWeight.isNotBlank() && editReps.toIntOrNull()?.let { it > 0 } == true
        Row(
            modifier = modifier.fillMaxWidth().padding(vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(modifier = Modifier.width(SET_COL_W)) {
                Text("%02d".format(setIndex), style = MaterialTheme.typography.labelSmall, color = muted)
            }
            OutlinedTextField(
                value = editWeight,
                onValueChange = { editWeight = it },
                modifier = Modifier.weight(1f),
                label = { Text("Weight") },
                singleLine = true,
                textStyle = MaterialTheme.typography.bodyMedium,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Next)
            )
            OutlinedTextField(
                value = editReps,
                onValueChange = { if (it.all { c -> c.isDigit() }) editReps = it },
                modifier = Modifier.width(REPS_COL_W + 24.dp),
                label = { Text("Reps") },
                singleLine = true,
                textStyle = MaterialTheme.typography.bodyMedium,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done)
            )
            IconButton(
                onClick = { if (canConfirm) { onEdit(editWeight.trim(), editReps.toInt()); isEditing = false } },
                modifier = Modifier.size(36.dp),
                enabled = canConfirm
            ) {
                Icon(Icons.Default.Check, contentDescription = "Confirm", tint = if (canConfirm) accent else muted.copy(0.4f))
            }
            IconButton(
                onClick = { editWeight = editSeed; editReps = set.reps.toString(); isEditing = false },
                modifier = Modifier.size(36.dp)
            ) {
                Icon(Icons.Default.Close, contentDescription = "Cancel", tint = muted)
            }
        }
    } else {
        val tapMod = if (onLongPress != null)
            modifier.combinedClickable(onClick = { isEditing = true }, onLongClick = onLongPress)
        else
            modifier.combinedClickable(onClick = { isEditing = true })

        // Build a single spoken description so TalkBack reads the row as one unit rather than
        // announcing each Text node separately: "Set 2, 50 kg, 10 reps[, RPE 8][, personal record]".
        val rowDescription = buildString {
            append("Set $setIndex")
            if (displayWeight != null) append(", $displayWeight")
            append(", ${set.reps} reps")
            set.rpe?.let { append(", RPE ${rpeLabel(it)}, ${rirLabel(it)} in reserve") }
            if (isPr) append(", personal record")
        }

        // Swipe the row left to delete this set — there's no other delete affordance. A custom
        // a11y action covers TalkBack, which can't perform the swipe gesture.
        val dismissState = rememberSwipeToDismissBoxState(
            confirmValueChange = { v ->
                // Trigger the delete but DON'T confirm the dismiss: the row stays put and is removed
                // only when the set list actually drops it. If the delete fails, the row snaps back
                // instead of vanishing while the set still lives in the DB.
                if (v == SwipeToDismissBoxValue.EndToStart) onDelete()
                false
            }
        )
        SwipeToDismissBox(
            state = dismissState,
            modifier = Modifier.fillMaxWidth(),
            enableDismissFromStartToEnd = false,
            backgroundContent = {
                SwipeDeleteBackground(active = dismissState.targetValue == SwipeToDismissBoxValue.EndToStart)
            }
        ) {
        Row(
            modifier = tapMod
                .semantics(mergeDescendants = true) {
                    contentDescription = rowDescription
                    customActions = listOf(CustomAccessibilityAction("Delete set") { onDelete(); true })
                }
                .fillMaxWidth()
                .graphicsLayer {
                    alpha = appear.value.coerceIn(0f, 1f)
                    translationY = (1f - appear.value) * 12.dp.toPx()
                    // PR rows start smaller and overshoot for a celebratory pop.
                    val s = if (isPr) 0.85f + 0.15f * appear.value else 0.96f + 0.04f * appear.value
                    scaleX = s
                    scaleY = s
                }
                .padding(vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            // Set number col (gold ★ inline next to number for set 1)
            Box(modifier = Modifier.width(SET_COL_W), contentAlignment = Alignment.TopStart) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text("%02d".format(setIndex), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                    // Gold ★ marks the actual record-setting set (the session's best PR), not a fixed row.
                    if (isPr) {
                        Text("★", style = MaterialTheme.typography.labelSmall, color = ForgePrGold, fontSize = 9.sp)
                    }
                }
            }

            // Weight + ghost prior col — gold only when this set is a new PR
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    displayWeight,
                    style = MaterialTheme.typography.headlineSmall,
                    color = if (isPr) ForgePrGold else onBg,
                    fontWeight = if (isPr) FontWeight.SemiBold else FontWeight.Normal
                )
                priorSet?.let { prior ->
                    // Plate exercises read as a plate count, not the lb equivalent (#9).
                    val priorDisplay = prior.weightLb?.let { lb ->
                        if (isPlates) "${formatPlateCount(lb / plateLb)} pl" else formatWeight(lb, useKg)
                    } ?: prior.weightText
                    Text(
                        "$priorDisplay × ${prior.reps}",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted.copy(alpha = 0.45f),
                        fontSize = 9.sp
                    )
                }
            }

            // Reps col
            Box(modifier = Modifier.width(REPS_COL_W), contentAlignment = Alignment.CenterStart) {
                Text("${set.reps}", style = MaterialTheme.typography.headlineSmall, color = onBg)
            }

            // RPE col — tappable framed box
            Box(modifier = Modifier.width(RPE_COL_W), contentAlignment = Alignment.Center) {
                if (onSetRpe != null) {
                    Box(
                        modifier = Modifier
                            .border(0.5.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(3.dp))
                            .combinedClickable(onClick = { showRpePicker = !showRpePicker }, onLongClick = { onSetRpe(null) })
                            .padding(horizontal = 8.dp, vertical = 3.dp)
                    ) {
                        Text(
                            set.rpe?.let { rpeLabel(it) } ?: "—",
                            style = MaterialTheme.typography.labelSmall,
                            color = if (set.rpe != null) onBg else muted.copy(alpha = 0.4f),
                            fontSize = 11.sp
                        )
                    }
                } else if (set.rpe != null) {
                    Text(rpeLabel(set.rpe), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 11.sp)
                }
            }

            // Delta col — trend icon + value, green when you beat last session
            Box(modifier = Modifier.width(DELTA_COL_W), contentAlignment = Alignment.CenterEnd) {
                deltaLabel?.let {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                        if (deltaPositive || deltaNegative) {
                            Icon(
                                if (deltaPositive) Icons.Filled.TrendingUp else Icons.Filled.TrendingDown,
                                contentDescription = null,
                                tint = deltaColor,
                                modifier = Modifier.size(13.dp)
                            )
                        }
                        Text(it, style = MaterialTheme.typography.labelSmall, color = deltaColor, fontSize = 9.sp)
                    }
                }
            }
        }
        }

        // System back closes the open picker instead of leaving the screen (the modal dialog this
        // replaced dismissed on back-press; the inline panel must honor it too).
        BackHandler(enabled = showRpePicker && onSetRpe != null) { showRpePicker = false }

        // Inline effort picker — slides in directly under the row (replaces the modal dialog),
        // showing each rating as both RPE and its reps-in-reserve equivalent.
        AnimatedVisibility(
            visible = showRpePicker && onSetRpe != null,
            enter = expandVertically(ForgeMotion.enterTween()) + fadeIn(ForgeMotion.enterTween()),
            exit = shrinkVertically(ForgeMotion.exitTween()) + fadeOut(ForgeMotion.exitTween())
        ) {
            InlineEffortPicker(
                current = set.rpe,
                onPick = { rpe -> onSetRpe?.invoke(rpe); showRpePicker = false },
                onClear = { onSetRpe?.invoke(null); showRpePicker = false },
                onDismiss = { showRpePicker = false }
            )
        }
    }
}

/** Red trailing-edge background revealed while swiping a set row left to delete it. */
@Composable
private fun SwipeDeleteBackground(active: Boolean) {
    Row(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.error.copy(alpha = if (active) 0.85f else 0.35f))
            .padding(horizontal = 20.dp),
        horizontalArrangement = Arrangement.End,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(Icons.Default.Delete, contentDescription = null, tint = Color.White, modifier = Modifier.size(18.dp))
    }
}

/**
 * Inline effort picker — RPE 6→10 in 0.5 steps, each chip labelled with both the RPE and its
 * reps-in-reserve equivalent so you can think in either. Replaces the old modal dialog; slides
 * in under the set row.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun InlineEffortPicker(
    current: Double?,
    onPick: (Double) -> Unit,
    onClear: () -> Unit,
    onDismiss: () -> Unit
) {
    val options = generateSequence(6.0) { it + 0.5 }.takeWhile { it <= 10.0 }.toList()
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    Column(modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(bottom = 6.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                "EFFORT · RPE / REPS IN RESERVE",
                style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp
            )
            // Always a visible dismiss: "clear" removes a set rating (and closes); "close" just
            // collapses the picker when nothing's rated yet (so it's never stuck open).
            if (current != null) {
                Text(
                    "clear",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.7f),
                    fontSize = 10.sp,
                    modifier = Modifier
                        .clickableLabeled("Clear effort rating") { onClear() }
                        .padding(horizontal = 4.dp, vertical = 2.dp)
                )
            } else {
                Text(
                    "close",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.7f),
                    fontSize = 10.sp,
                    modifier = Modifier
                        .clickableLabeled("Close effort picker") { onDismiss() }
                        .padding(horizontal = 4.dp, vertical = 2.dp)
                )
            }
        }
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            options.forEach { v ->
                val selected = current != null && kotlin.math.abs(current - v) < 0.01
                // Tapping a rating pops the chip with a spring overshoot — a tactile "got it" that
                // confirms the pick as the panel collapses. Reduced-motion-safe (bouncy() → snap()).
                val pop by animateFloatAsState(if (selected) 1.08f else 1f, ForgeMotion.bouncy(), label = "rpe-pop")
                Column(
                    modifier = Modifier
                        .graphicsLayer { scaleX = pop; scaleY = pop }
                        .border(
                            1.dp,
                            if (selected) onBg else outline.copy(alpha = 0.4f),
                            RoundedCornerShape(6.dp)
                        )
                        .clickableLabeled("RPE ${rpeLabel(v)}, ${rirLabel(v)} reps in reserve") { onPick(v) }
                        .sizeIn(minWidth = 44.dp)
                        .padding(horizontal = 10.dp, vertical = 6.dp),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    Text(rpeLabel(v), style = MaterialTheme.typography.bodyLarge, color = if (selected) onBg else muted)
                    Text(
                        "${rirLabel(v)} RIR",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted.copy(alpha = 0.6f),
                        fontSize = 8.sp
                    )
                }
            }
        }
    }
}
