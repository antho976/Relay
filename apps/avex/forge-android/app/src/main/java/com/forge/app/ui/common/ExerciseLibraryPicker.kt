package com.forge.app.ui.common

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.forge.app.program.ExerciseDef
import com.forge.app.program.ExerciseLibrary

/**
 * Pure, testable filter for the exercise picker. Hides owner-only plate-count stations
 * ([ExerciseDef.curatedOnly]) and anything already on the list ([exclude]); matches [query] against
 * the name or muscle group (case-insensitive). Blank query returns the whole eligible pool.
 */
fun filterLibrary(query: String, exclude: Set<String>): List<ExerciseDef> {
    val q = query.trim().lowercase()
    return ExerciseLibrary.all.filter { def ->
        !def.curatedOnly && def.id !in exclude &&
            (q.isEmpty() || def.name.lowercase().contains(q) || def.muscle.displayName.lowercase().contains(q))
    }
}

/**
 * Searchable, multi-select picker over the exercise library (machines included; owner-only
 * plate-count stations and already-present moves filtered out). Shared by the onboarding "make my
 * own" flow, the Program Editor, and the freestyle logger. Pictures are deferred — the row leaves
 * room for a thumbnail later.
 *
 * [title] heads the dialog; [confirmLabel] is the verb on the confirm button ("Add").
 */
@Composable
fun ExerciseLibraryPicker(
    exclude: Set<String>,
    onDismiss: () -> Unit,
    onConfirm: (Set<String>) -> Unit,
    title: String = "Add an exercise",
    confirmLabel: String = "Add"
) {
    var query by remember { mutableStateOf("") }
    var picked by remember { mutableStateOf(emptySet<String>()) }
    val results = remember(query, exclude) { filterLibrary(query, exclude) }

    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Surface(
            modifier = Modifier.padding(16.dp).fillMaxWidth().fillMaxHeight(0.85f),
            shape = RoundedCornerShape(16.dp),
            color = MaterialTheme.colorScheme.surface
        ) {
            Column(modifier = Modifier.fillMaxWidth().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(title, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onBackground)
                OutlinedTextField(
                    value = query, onValueChange = { query = it },
                    modifier = Modifier.fillMaxWidth(),
                    placeholder = { Text("Search exercises or muscles") },
                    singleLine = true
                )
                LazyColumn(
                    modifier = Modifier.fillMaxWidth().weight(1f),
                    verticalArrangement = Arrangement.spacedBy(2.dp)
                ) {
                    items(results, key = { it.id }) { def ->
                        val checked = def.id in picked
                        Row(
                            modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp))
                                .clickable { picked = if (checked) picked - def.id else picked + def.id }
                                .padding(vertical = 4.dp, horizontal = 4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            Checkbox(checked = checked, onCheckedChange = { picked = if (it) picked + def.id else picked - def.id })
                            Column(modifier = Modifier.weight(1f)) {
                                Text(def.name, style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurface)
                                Text(def.muscle.displayName, style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                    if (results.isEmpty()) {
                        item {
                            Text("No matches.", style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(8.dp))
                        }
                    }
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    TextButton(onClick = onDismiss) { Text("Cancel") }
                    Button(enabled = picked.isNotEmpty(), onClick = { onConfirm(picked) }) {
                        Text(if (picked.isEmpty()) confirmLabel else "$confirmLabel ${picked.size}")
                    }
                }
            }
        }
    }
}
