package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.LocalForgeSettings

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SetRow(
    set: LoggedSet,
    isPr: Boolean,
    onDelete: () -> Unit,
    onEdit: (weightText: String, reps: Int) -> Unit,
    /** Long-press to immediately log another set with the same weight and reps. */
    onLongPress: (() -> Unit)? = null,
    onToggleDifficultyTag: ((String?) -> Unit)? = null,
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

    val accent = MaterialTheme.colorScheme.primary
    val bg = if (isPr) accent.copy(alpha = 0.14f)
             else MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.4f)

    val baseModifier = modifier
        .fillMaxWidth()
        .clip(RoundedCornerShape(8.dp))
        .background(bg)
        .let { m -> if (isPr) m.border(1.dp, accent, RoundedCornerShape(8.dp)) else m }

    if (isEditing) {
        val canConfirm = editWeight.isNotBlank() && editReps.toIntOrNull()?.let { it > 0 } == true
        Row(
            modifier = baseModifier.padding(horizontal = 8.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            OutlinedTextField(
                value = editWeight,
                onValueChange = { editWeight = it },
                modifier = Modifier.weight(1.2f),
                label = { Text("Weight") },
                singleLine = true,
                textStyle = MaterialTheme.typography.bodyMedium,
                keyboardOptions = KeyboardOptions(
                    keyboardType = KeyboardType.Decimal,
                    imeAction = ImeAction.Next
                )
            )
            OutlinedTextField(
                value = editReps,
                onValueChange = { if (it.all { c -> c.isDigit() }) editReps = it },
                modifier = Modifier.weight(0.8f),
                label = { Text("Reps") },
                singleLine = true,
                textStyle = MaterialTheme.typography.bodyMedium,
                keyboardOptions = KeyboardOptions(
                    keyboardType = KeyboardType.Number,
                    imeAction = ImeAction.Done
                )
            )
            IconButton(
                onClick = {
                    if (canConfirm) {
                        onEdit(editWeight.trim(), editReps.toInt())
                        isEditing = false
                    }
                },
                modifier = Modifier.size(36.dp),
                enabled = canConfirm
            ) {
                Icon(
                    Icons.Default.Check,
                    contentDescription = "Confirm edit",
                    tint = if (canConfirm) accent else MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f)
                )
            }
            IconButton(
                onClick = {
                    editWeight = set.weightText
                    editReps = set.reps.toString()
                    isEditing = false
                },
                modifier = Modifier.size(36.dp)
            ) {
                Icon(
                    Icons.Default.Close,
                    contentDescription = "Cancel edit",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    } else {
        val compact = LocalForgeSettings.current.compactSetLogging
        val rowModifier = if (onLongPress != null) {
            baseModifier.combinedClickable(onClick = {}, onLongClick = onLongPress)
        } else {
            baseModifier
        }.padding(horizontal = 12.dp, vertical = if (compact) 2.dp else 6.dp)

        Row(
            modifier = rowModifier,
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Box(
                modifier = Modifier
                    .size(28.dp)
                    .clip(RoundedCornerShape(50))
                    .background(accent.copy(alpha = if (isPr) 0.4f else 0.2f)),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    "${set.setIndex + 1}",
                    color = accent,
                    fontWeight = FontWeight.Bold,
                    style = MaterialTheme.typography.labelLarge
                )
            }
            Text(
                "$displayWeight × ${set.reps}",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.weight(1f)
            )
            onToggleDifficultyTag?.let { toggle ->
                val tagLabel = when (set.difficultyTag) { "easy" -> "E" "hard" -> "H" else -> "·" }
                val tagColor = when (set.difficultyTag) {
                    "easy" -> MaterialTheme.colorScheme.tertiary
                    "hard" -> MaterialTheme.colorScheme.error
                    else -> MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.3f)
                }
                Text(
                    tagLabel,
                    style = MaterialTheme.typography.labelSmall,
                    color = tagColor,
                    modifier = Modifier
                        .bounceClick { toggle(set.difficultyTag) }
                        .padding(horizontal = 4.dp)
                )
            }
            // Set annotation badges (#140, #141, #18)
            if (set.isAmrap) {
                AnnotationBadge("AMRAP", MaterialTheme.colorScheme.tertiary,
                    onClick = onToggleAmrap)
            }
            if (set.isAssisted) {
                AnnotationBadge("ASSIST", MaterialTheme.colorScheme.secondary,
                    onClick = onToggleAssisted)
            }
            if (set.toFailure) {
                AnnotationBadge("FAIL", MaterialTheme.colorScheme.error,
                    onClick = onToggleFailure)
            }
            set.setType?.let { type ->
                AnnotationBadge(type.uppercase().take(4), MaterialTheme.colorScheme.outline, null)
            }
            if (isPr) {
                Text(
                    "PR",
                    color = accent,
                    fontWeight = FontWeight.Black,
                    style = MaterialTheme.typography.labelLarge
                )
            }
            IconButton(
                onClick = {
                    editWeight = set.weightText
                    editReps = set.reps.toString()
                    isEditing = true
                },
                modifier = Modifier.size(28.dp)
            ) {
                Icon(
                    Icons.Default.Edit,
                    contentDescription = "Edit set",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            IconButton(
                onClick = onDelete,
                modifier = Modifier.size(28.dp)
            ) {
                Icon(
                    Icons.Default.Close,
                    contentDescription = "Delete set",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@Composable
private fun AnnotationBadge(label: String, color: androidx.compose.ui.graphics.Color, onClick: (() -> Unit)?) {
    val m = if (onClick != null)
        androidx.compose.ui.Modifier.bounceClick { onClick() }
    else androidx.compose.ui.Modifier
    Text(label, style = MaterialTheme.typography.labelSmall,
        color = color, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold,
        modifier = m.padding(horizontal = 3.dp))
}