package com.forge.app.ui.gym.train.components

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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.ForgePrGold
import com.forge.app.ui.theme.LocalForgeSettings

private val SET_COL_W = 36.dp
private val REPS_COL_W = 48.dp
private val RPE_COL_W = 44.dp
private val DELTA_COL_W = 72.dp

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SetRow(
    set: LoggedSet,
    setIndex: Int = 1,
    isPr: Boolean,
    priorSet: LoggedSet? = null,
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
    val displayWeight = set.weightLb?.let { formatWeight(it, useKg) } ?: set.weightText
    var isEditing by remember(set.id) { mutableStateOf(false) }
    var editWeight by remember { mutableStateOf(set.weightText) }
    var editReps by remember { mutableStateOf(set.reps.toString()) }
    var showRpePicker by remember { mutableStateOf(false) }

    val accent = MaterialTheme.colorScheme.primary
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline

    val wLb = set.weightLb ?: 0.0
    val pLb = priorSet?.weightLb ?: 0.0
    val wDiff = wLb - pLb
    val rDiff = if (priorSet != null) set.reps - priorSet.reps else 0
    val deltaLabel = if (priorSet != null) when {
        wDiff >= 0.5 -> "+${wDiff.toInt()} lb"
        wDiff <= -0.5 -> "${wDiff.toInt()} lb"
        rDiff > 0 -> "+$rDiff rep"
        rDiff < 0 -> "$rDiff rep"
        else -> null
    } else null
    val deltaPositive = wDiff >= 0.5 || (wDiff > -0.5 && rDiff > 0)
    val deltaColor = if (deltaPositive) ForgeLastGreen else muted.copy(alpha = 0.7f)

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
                onClick = { editWeight = set.weightText; editReps = set.reps.toString(); isEditing = false },
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
            modifier = tapMod.fillMaxWidth().padding(vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            // Set number col (gold ★ inline next to number for set 1)
            Box(modifier = Modifier.width(SET_COL_W), contentAlignment = Alignment.TopStart) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text("%02d".format(setIndex), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                    if (setIndex == 1) {
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
                    val priorDisplay = prior.weightLb?.let { formatWeight(it, useKg) } ?: prior.weightText
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
                        Icon(
                            if (deltaPositive) Icons.Filled.TrendingUp else Icons.Filled.TrendingDown,
                            contentDescription = null,
                            tint = deltaColor,
                            modifier = Modifier.size(13.dp)
                        )
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

private fun rpeLabel(rpe: Double): String = if (rpe % 1.0 == 0.0) "${rpe.toInt()}" else "%.1f".format(rpe)

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
