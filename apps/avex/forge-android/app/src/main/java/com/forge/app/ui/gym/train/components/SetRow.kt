package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.formatWeight
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

    if (isEditing) {
        val canConfirm = editWeight.isNotBlank() && editReps.toIntOrNull()?.let { it > 0 } == true
        Row(
            modifier = modifier.fillMaxWidth().padding(vertical = 4.dp),
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
        val tapModifier = if (onLongPress != null) {
            modifier.combinedClickable(
                onClick = { isEditing = true },
                onLongClick = onLongPress
            )
        } else {
            modifier.clickable { isEditing = true }
        }

        Column(modifier = tapModifier.padding(vertical = 2.dp)) {
            Text(
                "$displayWeight × ${set.reps}",
                style = MaterialTheme.typography.headlineSmall,
                color = if (isPr) accent else MaterialTheme.colorScheme.onBackground,
                fontWeight = if (isPr) FontWeight.SemiBold else FontWeight.Normal
            )
            if (isPr) {
                Text(
                    "PR",
                    style = MaterialTheme.typography.labelSmall,
                    color = accent,
                    fontWeight = FontWeight.Black
                )
            }
        }
    }
}
