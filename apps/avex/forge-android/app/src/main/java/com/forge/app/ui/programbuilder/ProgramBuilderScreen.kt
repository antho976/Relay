@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
package com.forge.app.ui.programbuilder

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.DragHandle
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.forge.app.ui.common.DraggableItem
import com.forge.app.ui.common.dragContainer
import com.forge.app.ui.common.rememberDragDropState

/**
 * Routine builder — add/rename/reorder/remove days and (in the day detail) their exercises, then
 * Save. Opens blank for build-your-own (from onboarding) or pre-loaded to edit the current plan.
 * Two levels in one screen via local state: the day list and a single day's detail.
 */
@Composable
fun ProgramBuilderScreen(
    blank: Boolean,
    onClose: () -> Unit,
    viewModel: ProgramBuilderViewModel = hiltViewModel()
) {
    LaunchedEffect(Unit) { viewModel.loadIfNeeded(blank) }
    var editingDayUid by remember { mutableStateOf<String?>(null) }
    var showDiscard by remember { mutableStateOf(false) }

    val editingDay = editingDayUid?.let { viewModel.day(it) }
    if (editingDay != null) {
        ProgramBuilderDayDetail(
            day = editingDay,
            onBack = { editingDayUid = null },
            onRename = { viewModel.renameDay(editingDay.uid, it) },
            onSetType = { viewModel.setDayType(editingDay.uid, it) },
            onSetAccent = { viewModel.setDayAccent(editingDay.uid, it) },
            onAddExercises = { viewModel.addExercises(editingDay.uid, it) },
            onRemoveExercise = { viewModel.removeExercise(editingDay.uid, it) },
            onMoveExercise = { from, to -> viewModel.moveExercise(editingDay.uid, from, to) },
            onSetExercise = { exUid, sets, reps -> viewModel.setExercise(editingDay.uid, exUid, sets, reps) }
        )
        return
    }

    fun attemptClose() { if (viewModel.dirty) showDiscard = true else onClose() }
    BackHandler { attemptClose() }

    val days = viewModel.days
    val listState = rememberLazyListState()
    // One leading item (the "tap a day / press-and-hold to drag" hint) sits above the draggable rows.
    val dragState = rememberDragDropState(listState, firstDraggableIndex = 1) { from, to -> viewModel.moveDay(from, to) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text("BUILD YOUR PLAN", style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Text("Your days", style = MaterialTheme.typography.titleLarge,
                            fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.onBackground)
                    }
                },
                navigationIcon = { IconButton(onClick = { attemptClose() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                actions = {
                    TextButton(onClick = { viewModel.save { onClose() } }, enabled = !viewModel.saving) {
                        Text("Save", fontWeight = FontWeight.SemiBold)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        bottomBar = {
            Button(onClick = { viewModel.addDay() }, modifier = Modifier.fillMaxWidth().padding(16.dp)) {
                Icon(Icons.Default.Add, null); Text("  Add day")
            }
        },
        containerColor = Color.Transparent
    ) { inner ->
        if (days.isEmpty()) {
            Column(
                modifier = Modifier.fillMaxSize().padding(inner).padding(horizontal = 24.dp),
                verticalArrangement = Arrangement.Center
            ) {
                Text("No days yet.", style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onBackground)
                Text("Tap “Add day” to start building your plan. Add as many as you like, then fill each with exercises.",
                    style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        } else {
            LazyColumn(
                state = listState,
                modifier = Modifier.fillMaxSize().padding(inner).dragContainer(dragState),
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                item {
                    Text("Tap a day to edit it. Press and hold to drag it into order.",
                        style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                itemsIndexed(days, key = { _, d -> d.uid }) { index, d ->
                    DraggableItem(dragState, index) { dragging ->
                        DayRow(
                            name = d.name,
                            subtitle = "${typeLabel(d.archetype)} · ${d.exercises.size} exercises",
                            accentHex = d.accentHex,
                            elevated = dragging,
                            onClick = { editingDayUid = d.uid },
                            onDelete = { viewModel.removeDay(d.uid) }
                        )
                    }
                }
            }
        }
    }

    if (showDiscard) {
        AlertDialog(
            onDismissRequest = { showDiscard = false },
            title = { Text("Discard changes?") },
            text = { Text("Your edits to this plan haven't been saved.") },
            confirmButton = { TextButton(onClick = { showDiscard = false; onClose() }) { Text("Discard") } },
            dismissButton = { TextButton(onClick = { showDiscard = false }) { Text("Keep editing") } }
        )
    }
}

@Composable
private fun DayRow(
    name: String,
    subtitle: String,
    accentHex: String,
    elevated: Boolean,
    onClick: () -> Unit,
    onDelete: () -> Unit
) {
    val accent = remember(accentHex) {
        runCatching { Color(android.graphics.Color.parseColor(accentHex)) }.getOrDefault(Color.Gray)
    }
    Row(
        modifier = Modifier.fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = if (elevated) 0.9f else 0.5f))
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Icon(Icons.Default.DragHandle, "Drag to reorder", tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Box(Modifier.size(10.dp).clip(CircleShape).background(accent))
        Column(Modifier.weight(1f)) {
            Text(name, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface)
            Text(subtitle, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        IconButton(onClick = onDelete) {
            Icon(Icons.Default.Delete, "Remove day", tint = MaterialTheme.colorScheme.error.copy(alpha = 0.7f))
        }
    }
}
