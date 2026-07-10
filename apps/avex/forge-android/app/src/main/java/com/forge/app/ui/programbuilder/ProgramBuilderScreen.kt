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
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
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
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.DraggableItem
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.GlyphButton
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
    var showFreestyleSwitch by remember { mutableStateOf(false) }
    val freestyleMode by viewModel.freestyleMode.collectAsStateWithLifecycle()

    // Saving a plan flips a "go with the flow" user to follow-a-plan — confirm that switch first so it's
    // never silent. Plan users save straight through.
    fun attemptSave() { if (freestyleMode) showFreestyleSwitch = true else viewModel.save { onClose() } }

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
                // §2: wordmark + back, never the screen's name — "Your days" heads the content.
                title = { ForgeWordmark() },
                navigationIcon = { IconButton(onClick = { attemptClose() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        bottomBar = {
            // §8: the page-level actions group at the END — filled do-it-now + its outlined sidekick.
            Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 16.dp)) {
                ForgeOutlineCapsule("+ Add day", onClick = { viewModel.addDay() }, modifier = Modifier.fillMaxWidth())
                Spacer(Modifier.height(8.dp))
                ForgePrimaryCapsule(
                    "Save",
                    onClick = { attemptSave() },
                    enabled = !viewModel.saving,
                    modifier = Modifier.fillMaxWidth()
                )
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
                contentPadding = PaddingValues(horizontal = 24.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                // ONE leading item (headline + hint) so firstDraggableIndex = 1 stays true.
                item {
                    Column {
                        Text("Your days", style = MaterialTheme.typography.headlineSmall,
                            color = MaterialTheme.colorScheme.onBackground)
                        Spacer(Modifier.height(8.dp))
                        Text("Tap a day to edit it. Press and hold to drag it into order.",
                            style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
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

    if (showFreestyleSwitch) {
        AlertDialog(
            onDismissRequest = { showFreestyleSwitch = false },
            title = { Text("Switch to following a plan?") },
            text = {
                Text("Saving this plan turns off Go with the flow and starts you on it. You can switch " +
                    "back to free logging anytime in Settings → Program.")
            },
            confirmButton = {
                TextButton(onClick = { showFreestyleSwitch = false; viewModel.save { onClose() } }) {
                    Text("Save & follow")
                }
            },
            dismissButton = { TextButton(onClick = { showFreestyleSwitch = false }) { Text("Cancel") } }
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
        // Text glyph over a stock icon (§8) — the whole row long-presses to drag.
        Text("≡", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Box(Modifier.size(10.dp).clip(CircleShape).background(accent))
        Column(Modifier.weight(1f)) {
            Text(name, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface)
            Text(subtitle, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        // Error at full strength (§5: error mirrors accent, no dimmed decoration); ≥48dp target.
        GlyphButton("×", "Remove day", MaterialTheme.colorScheme.error, onClick = onDelete)
    }
}
