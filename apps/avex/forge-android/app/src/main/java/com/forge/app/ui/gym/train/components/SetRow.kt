package com.forge.app.ui.gym.train.components

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.TrendingDown
import androidx.compose.material.icons.filled.TrendingUp
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.formatWeightDelta
import com.forge.app.domain.units.unitLabel
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

        Row(
            modifier = tapMod
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
                            .combinedClickable(onClick = { showRpePicker = true }, onLongClick = { onSetRpe(null) })
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

        if (showRpePicker && onSetRpe != null) {
            RpePickerDialog(
                current = set.rpe,
                onPick = { rpe -> onSetRpe(rpe); showRpePicker = false },
                onClear = { onSetRpe(null); showRpePicker = false },
                onDismiss = { showRpePicker = false }
            )
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun RpePickerDialog(
    current: Double?,
    onPick: (Double) -> Unit,
    onClear: () -> Unit,
    onDismiss: () -> Unit
) {
    val options = generateSequence(6.0) { it + 0.5 }.takeWhile { it <= 10.0 }.toList()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("RPE — how hard was that set?") },
        text = {
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                options.forEach { v ->
                    val selected = current != null && kotlin.math.abs(current - v) < 0.01
                    Box(
                        modifier = Modifier
                            .border(
                                1.dp,
                                if (selected) MaterialTheme.colorScheme.onSurface
                                else MaterialTheme.colorScheme.outline.copy(alpha = 0.4f),
                                RoundedCornerShape(4.dp)
                            )
                            .clickable { onPick(v) }
                            .padding(horizontal = 14.dp, vertical = 10.dp)
                    ) { Text(rpeLabel(v), style = MaterialTheme.typography.bodyMedium) }
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            if (current != null) {
                TextButton(onClick = onClear) { Text("Clear") }
            } else {
                TextButton(onClick = onDismiss) { Text("Cancel") }
            }
        }
    )
}
