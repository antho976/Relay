@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class, androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.programbuilder

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.DraggableItem
import com.forge.app.ui.common.ExerciseLibraryPicker
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.GlyphButton
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.dragContainer
import com.forge.app.ui.common.rememberDragDropState

@Composable
fun ProgramBuilderDayDetail(
    day: BuilderDay,
    onBack: () -> Unit,
    onRename: (String) -> Unit,
    onSetType: (String) -> Unit,
    onSetAccent: (String) -> Unit,
    onAddExercises: (Collection<String>) -> Unit,
    onRemoveExercise: (String) -> Unit,
    onMoveExercise: (Int, Int) -> Unit,
    onSetExercise: (String, Int, String) -> Unit
) {
    var showRename by remember { mutableStateOf(false) }
    var showPicker by remember { mutableStateOf(false) }
    var editExercise by remember { mutableStateOf<BuilderExercise?>(null) }

    val listState = rememberLazyListState()
    // Four leading items (day name, TYPE picker, COLOR picker, hint) sit above the draggable exercise rows.
    val dragState = rememberDragDropState(listState, firstDraggableIndex = 4) { from, to -> onMoveExercise(from, to) }

    Scaffold(
        topBar = {
            TopAppBar(
                // §2: wordmark + back, never the screen's name — the day name heads the content.
                title = { ForgeWordmark() },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        bottomBar = {
            ForgeOutlineCapsule(
                "+ Add exercise",
                onClick = { showPicker = true },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 16.dp)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize().padding(inner).dragContainer(dragState),
            contentPadding = PaddingValues(horizontal = 24.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            item {
                // One affordance: the name itself renames (the old separate Rename button doubled it).
                Text(
                    day.name,
                    style = MaterialTheme.typography.headlineSmall,
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.clickableLabeled("Rename day") { showRename = true }.padding(vertical = 4.dp)
                )
            }
            item {
                Text("TYPE", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    DAY_TYPES.forEach { (key, label, _) ->
                        // Normalize so a stored "upper-a"/"lower-b" still selects its base-type pill.
                        SegmentPill(
                            text = label,
                            selected = baseArchetype(day.archetype) == key,
                            onClick = { onSetType(key) },
                            accent = MaterialTheme.colorScheme.primary,
                            onBg = MaterialTheme.colorScheme.onBackground,
                            muted = MaterialTheme.colorScheme.onSurfaceVariant,
                            outline = MaterialTheme.colorScheme.outline
                        )
                    }
                }
            }
            item {
                Text("COLOR", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    DAY_ACCENTS.forEach { hex ->
                        val c = runCatching { Color(android.graphics.Color.parseColor(hex)) }.getOrDefault(Color.Gray)
                        val selected = hex.equals(day.accentHex, ignoreCase = true)
                        Box(
                            Modifier.size(28.dp).clip(CircleShape).background(c)
                                .border(
                                    width = if (selected) 3.dp else 0.dp,
                                    color = MaterialTheme.colorScheme.onBackground, shape = CircleShape
                                )
                                .clickable { onSetAccent(hex) }
                        )
                    }
                }
            }
            item {
                Text(
                    if (day.exercises.isEmpty()) "No exercises yet. Add one below."
                    else "Tap an exercise for sets/reps. Press and hold to drag it into order.",
                    style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 6.dp)
                )
            }
            itemsIndexed(day.exercises, key = { _, e -> e.uid }) { index, e ->
                DraggableItem(dragState, index) { dragging ->
                    Row(
                        modifier = Modifier.fillMaxWidth()
                            .clip(RoundedCornerShape(12.dp))
                            .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = if (dragging) 0.9f else 0.5f))
                            .clickable { editExercise = e }
                            .padding(horizontal = 12.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp)
                    ) {
                        // Text glyph over a stock icon (§8) — the whole row long-presses to drag.
                        Text("≡", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Column(Modifier.weight(1f)) {
                            Text(e.name, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold,
                                color = MaterialTheme.colorScheme.onSurface)
                            Text("${e.muscle} · ${e.sets} sets · ${e.reps}",
                                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        // Error at full strength (§5); GlyphButton guarantees the ≥48dp touch target.
                        GlyphButton(
                            "×", "Remove ${e.name}", MaterialTheme.colorScheme.error,
                            onClick = { onRemoveExercise(e.uid) }
                        )
                    }
                }
            }
        }
    }

    if (showRename) {
        var text by remember { mutableStateOf(day.name) }
        AlertDialog(
            onDismissRequest = { showRename = false },
            title = { Text("Day name") },
            text = {
                OutlinedTextField(
                    value = text, onValueChange = { text = it.take(MAX_DAY_NAME) }, singleLine = true,
                    supportingText = { Text("${text.length}/$MAX_DAY_NAME") }
                )
            },
            confirmButton = { TextButton(onClick = { onRename(text.trim().ifBlank { day.name }); showRename = false }) { Text("Save") } },
            dismissButton = { TextButton(onClick = { showRename = false }) { Text("Cancel") } }
        )
    }

    editExercise?.let { ex ->
        SetsRepsDialog(
            name = ex.name, sets = ex.sets, reps = ex.reps,
            onDismiss = { editExercise = null },
            onSave = { sets, reps -> onSetExercise(ex.uid, sets, reps); editExercise = null }
        )
    }

    if (showPicker) {
        ExerciseLibraryPicker(
            exclude = day.exercises.map { it.libId }.toSet(),
            onDismiss = { showPicker = false },
            onConfirm = { picked -> onAddExercises(picked); showPicker = false }
        )
    }
}

@Composable
private fun SetsRepsDialog(name: String, sets: Int, reps: String, onDismiss: () -> Unit, onSave: (Int, String) -> Unit) {
    var setsText by remember { mutableStateOf(sets.toString()) }
    var repsText by remember { mutableStateOf(reps) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(name) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = setsText, onValueChange = { setsText = it.filter { c -> c.isDigit() } },
                    label = { Text("Sets") }, singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number)
                )
                OutlinedTextField(
                    value = repsText, onValueChange = { repsText = it },
                    label = { Text("Reps (e.g. 8-12)") }, singleLine = true
                )
            }
        },
        confirmButton = {
            TextButton(onClick = { onSave(setsText.toIntOrNull()?.coerceIn(1, 20) ?: sets, repsText.trim().ifBlank { reps }) }) {
                Text("Save")
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}
