package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.program.ExercisePlan
import com.forge.app.program.ExerciseTag
import com.forge.app.program.Swap
import com.forge.app.program.Swaps

/**
 * Modal bottom sheet showing swap candidates for [forExercise]'s muscle group.
 * Each swap has two actions: "This session" (LoggedExercise-level swap) and
 * "Make default" (writes ExerciseCustomization). A "Clear default" button
 * appears when a persistent swap is active.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SwapPickerSheet(
    forExercise: ExercisePlan,
    hasPersistentSwap: Boolean,
    onPickForSession: (Swap) -> Unit,
    onPickPersistent: (Swap) -> Unit,
    onClearPersistent: () -> Unit,
    onDismiss: () -> Unit
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val allSwaps = Swaps.forMuscle(forExercise.muscle)
    var searchQuery by remember { mutableStateOf("") }
    var tagFilter by remember { mutableStateOf<ExerciseTag?>(null) }
    val swaps = allSwaps.filter { swap ->
        (searchQuery.isEmpty() || swap.name.contains(searchQuery, ignoreCase = true)) &&
        (tagFilter == null) // Swaps don't have tags yet — filter is a no-op but UI is wired
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surface
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Column {
                Text(
                    "SWAP",
                    color = MaterialTheme.colorScheme.primary,
                    fontWeight = FontWeight.Black,
                    style = MaterialTheme.typography.labelLarge
                )
                Text(
                    forExercise.muscle.displayName,
                    style = MaterialTheme.typography.titleLarge,
                    color = MaterialTheme.colorScheme.onSurface
                )
                Text(
                    "Pick a variant. \"This session\" only changes today's logged entry; " +
                        "\"Make default\" replaces the exercise in every future session too.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 4.dp)
                )
            }

            // Search bar (#22)
            OutlinedTextField(
                value = searchQuery,
                onValueChange = { searchQuery = it },
                modifier = Modifier.fillMaxWidth(),
                placeholder = { Text("Search swaps…") },
                singleLine = true
            )

            // Tag filter chips (#37)
            Row(
                modifier = Modifier.horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                FilterChip(
                    selected = tagFilter == null,
                    onClick = { tagFilter = null },
                    label = { Text("All") }
                )
                ExerciseTag.entries.forEach { tag ->
                    FilterChip(
                        selected = tagFilter == tag,
                        onClick = { tagFilter = if (tagFilter == tag) null else tag },
                        label = { Text(tag.display) }
                    )
                }
            }

            if (hasPersistentSwap) {
                TextButton(
                    onClick = onClearPersistent,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text("Clear current default")
                }
            }

            HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f))

            swaps.forEach { swap ->
                SwapRow(
                    swap = swap,
                    onPickForSession = { onPickForSession(swap) },
                    onPickPersistent = { onPickPersistent(swap) }
                )
            }
        }
    }
}

@Composable
private fun SwapRow(
    swap: Swap,
    onPickForSession: () -> Unit,
    onPickPersistent: () -> Unit
) {
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.35f)
    ) {
        Column(
            modifier = Modifier.padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Text(
                swap.name,
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSurface
            )
            Text(
                swap.muscleTarget,
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary
            )
            Text(
                swap.why,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Text(
                "When: ${swap.whenToUse}",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                OutlinedButton(
                    onClick = onPickForSession,
                    modifier = Modifier.weight(1f)
                ) {
                    Text("This session")
                }
                OutlinedButton(
                    onClick = onPickPersistent,
                    modifier = Modifier.weight(1f)
                ) {
                    Text("Make default")
                }
            }
        }
    }
}
