package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.ui.Alignment
import com.forge.app.ui.common.ForgeGroupLabel
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.clickableLabeled
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import com.forge.app.ui.common.window.ModalBottomSheet
import androidx.compose.material3.Text
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
import com.forge.app.program.Program

/**
 * Full-catalog exercise picker shown during an active session (#61).
 * Exercises are grouped by muscle group with a live text filter.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AddExerciseSheet(
    alreadyAddedIds: Set<String>,
    onPick: (String) -> Unit,
    onDismiss: () -> Unit
) {
    val sheetState = rememberModalBottomSheetState()
    var query by remember { mutableStateOf("") }

    val allExercises: List<ExercisePlan> = remember {
        Program.days.flatMap { it.exercises }.distinctBy { it.id }
    }
    val filtered = if (query.isBlank()) allExercises
                   else allExercises.filter { it.name.contains(query, ignoreCase = true) }
    val grouped = filtered.groupBy { it.muscle.displayName }.toSortedMap()

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surface
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(0.dp)
        ) {
            Text(
                "ADD EXERCISE",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Black,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.padding(horizontal = 20.dp, vertical = 12.dp)
            )
            GymSearchRow(
                query = query,
                onQueryChange = { query = it },
                placeholder = "Search exercises",
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)
            )
            // Each muscle is one group of filled rows, 2dp seams, no lines.
            LazyColumn(
                modifier = Modifier.fillMaxWidth(),
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp)
            ) {
                grouped.forEach { (muscle, exercises) ->
                    item(key = "h-$muscle") {
                        Box(Modifier.padding(start = 4.dp, end = 4.dp, top = 18.dp, bottom = 10.dp)) {
                            ForgeGroupLabel(muscle)
                        }
                    }
                    itemsIndexed(exercises, key = { _, it -> it.id }) { i, plan ->
                        val alreadyAdded = plan.id in alreadyAddedIds
                        val dim = if (alreadyAdded) 0.35f else 1f
                        Column {
                            if (i > 0) Spacer(Modifier.height(GROUP_SEAM))
                            Row(
                                modifier = Modifier
                                    .groupMember(i, exercises.size)
                                    .then(
                                        if (alreadyAdded) Modifier
                                        else Modifier.clickableLabeled("Add ${plan.name}") { onPick(plan.id) }
                                    )
                                    .heightIn(min = 52.dp)
                                    .padding(horizontal = ROW_H, vertical = 14.dp),
                                horizontalArrangement = Arrangement.spacedBy(12.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    plan.name,
                                    style = MaterialTheme.typography.bodyLarge,
                                    color = MaterialTheme.colorScheme.onSurface.copy(alpha = dim),
                                    modifier = Modifier.weight(1f)
                                )
                                if (alreadyAdded) {
                                    Text(
                                        "ADDED",
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
