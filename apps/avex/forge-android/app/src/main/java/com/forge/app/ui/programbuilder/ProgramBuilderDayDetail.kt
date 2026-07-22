@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class, androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.programbuilder

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.program.ExerciseLibrary
import com.forge.app.ui.common.DraggableItem
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ExerciseIcons
import com.forge.app.ui.common.ExerciseLibraryPicker
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.GlyphButton
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.dragContainer
import com.forge.app.ui.common.parseAccentHex
import com.forge.app.ui.common.rememberDragDropState

/**
 * One day of the plan, editable: rename (the serif name itself), type + colour, and the exercise
 * list — tap a row for its sets × reps sheet (steppers + rep presets + in-place swap), long-press-drag
 * to reorder, × to remove (undone via the shared snackbar). Day-level one-shots (add / duplicate /
 * remove) group at the page end (§3).
 */
@Composable
fun ProgramBuilderDayDetail(
    day: BuilderDay,
    snackbarHostState: SnackbarHostState,
    onBack: () -> Unit,
    onRename: (String) -> Unit,
    onSetType: (String) -> Unit,
    onSetAccent: (String) -> Unit,
    onAddExercises: (Collection<String>) -> Unit,
    onRemoveExercise: (String) -> Unit,
    onSwapExercise: (String, String) -> Unit,
    onMoveExercise: (Int, Int) -> Unit,
    onSetExercise: (String, Int, String) -> Unit,
    onDuplicateDay: () -> Unit,
    onRemoveDay: () -> Unit
) {
    var showRename by remember { mutableStateOf(false) }
    var showAddPicker by remember { mutableStateOf(false) }
    // uid, not a snapshot: the sheet re-reads the live exercise so stepper taps render immediately.
    var sheetExerciseUid by remember { mutableStateOf<String?>(null) }
    var swapExerciseUid by remember { mutableStateOf<String?>(null) }

    // System back steps out of the day, not out of the whole builder.
    BackHandler { onBack() }

    val listState = rememberLazyListState()
    // Four leading items (name, TYPE, COLOR, EXERCISES anchor) sit above the draggable rows.
    val dragState = rememberDragDropState(listState, firstDraggableIndex = 4) { from, to -> onMoveExercise(from, to) }

    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground

    Scaffold(
        topBar = {
            TopAppBar(
                // §2: wordmark + back, never the screen's name — the day name heads the content.
                title = { ForgeWordmark() },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        snackbarHost = { SnackbarHost(snackbarHostState) },
        bottomBar = {
            // §3: this page's one-shots at the END — add (do-it-now) + duplicate / remove sidekicks.
            Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 16.dp)) {
                ForgePrimaryCapsule("+ Add exercise", onClick = { showAddPicker = true }, modifier = Modifier.fillMaxWidth())
                Spacer(Modifier.height(8.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    ForgeOutlineCapsule("Duplicate day", onClick = onDuplicateDay, modifier = Modifier.weight(1f))
                    ForgeOutlineCapsule(
                        "Remove day",
                        onClick = onRemoveDay,
                        modifier = Modifier.weight(1f),
                        contentColor = MaterialTheme.colorScheme.error
                    )
                }
            }
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize().padding(inner).dragContainer(dragState),
            contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 8.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            item(key = "name") {
                // One affordance: the name itself renames; the pencil glyph makes it discoverable.
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier
                        .bounceCombinedClick(onClickLabel = "Rename day", onClick = { showRename = true })
                        .padding(vertical = 4.dp)
                ) {
                    Text(day.name, style = MaterialTheme.typography.headlineSmall, color = onBg,
                        maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                    Spacer(Modifier.width(10.dp))
                    Text("✎", style = MaterialTheme.typography.titleMedium, color = muted)
                }
            }
            item(key = "type") {
                Column(Modifier.padding(top = 12.dp)) {
                    EditorialHeader("Type", muted = muted, accent = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.height(10.dp))
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        DAY_TYPES.forEach { (key, label, _) ->
                            // Normalize so a stored "upper-a"/"lower-b" still selects its base-type pill.
                            SegmentPill(
                                text = label,
                                selected = baseArchetype(day.archetype) == key,
                                onClick = { onSetType(key) },
                                accent = MaterialTheme.colorScheme.primary,
                                onBg = onBg,
                                muted = muted,
                                outline = MaterialTheme.colorScheme.outline
                            )
                        }
                    }
                }
            }
            item(key = "color") {
                Column(Modifier.padding(top = 12.dp)) {
                    EditorialHeader("Color", muted = muted, accent = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.height(2.dp))
                    Row {
                        DAY_ACCENTS.forEachIndexed { i, hex ->
                            val chosen = hex.equals(day.accentHex, ignoreCase = true)
                            // 48dp wrapper = the real touch target; the 28dp swatch stays trim (§8).
                            Box(
                                Modifier.minimumInteractiveComponentSize()
                                    .bounceCombinedClick(onClickLabel = "Day color ${i + 1}", onClick = { onSetAccent(hex) })
                                    .semantics { selected = chosen },
                                contentAlignment = Alignment.Center
                            ) {
                                Box(
                                    Modifier.size(28.dp).clip(CircleShape).background(parseAccentHex(hex))
                                        .then(if (chosen) Modifier.border(2.dp, onBg, CircleShape) else Modifier)
                                )
                            }
                        }
                    }
                }
            }
            item(key = "exercises") {
                Column(Modifier.padding(top = 12.dp)) {
                    Row(
                        Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            "EXERCISES",
                            style = MaterialTheme.typography.labelLarge,
                            color = muted,
                            letterSpacing = 1.sp,
                            modifier = Modifier.semantics { heading() }
                        )
                        Text("${day.totalSets} SETS", style = MaterialTheme.typography.labelSmall, color = muted)
                    }
                    Spacer(Modifier.height(2.dp))
                    if (day.exercises.isEmpty()) {
                        InlineEmptyHint("No exercises yet. Add one below.", muted.copy(alpha = 0.7f))
                    } else {
                        Text("Tap for sets and reps. Hold to reorder.",
                            style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.7f))
                    }
                }
            }
            itemsIndexed(day.exercises, key = { _, e -> e.uid }) { index, e ->
                DraggableItem(dragState, index) { dragging ->
                    ExerciseRow(
                        exercise = e,
                        dragging = dragging,
                        onOpen = { sheetExerciseUid = e.uid },
                        onRemove = { onRemoveExercise(e.uid) }
                    )
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

    // Re-read the live exercise each composition so stepper/pill edits render as they land; a swap
    // or removal that drops the uid simply closes the sheet.
    val sheetExercise = sheetExerciseUid?.let { uid -> day.exercises.firstOrNull { it.uid == uid } }
    if (sheetExercise != null) {
        SetsRepsSheet(
            exercise = sheetExercise,
            onSet = { sets, reps -> onSetExercise(sheetExercise.uid, sets, reps) },
            onSwap = { swapExerciseUid = sheetExercise.uid; sheetExerciseUid = null },
            onDismiss = { sheetExerciseUid = null }
        )
    }

    if (showAddPicker) {
        ExerciseLibraryPicker(
            exclude = day.exercises.map { it.libId }.toSet(),
            onDismiss = { showAddPicker = false },
            onConfirm = { picked -> onAddExercises(picked); showAddPicker = false }
        )
    }

    val swapExercise = swapExerciseUid?.let { uid -> day.exercises.firstOrNull { it.uid == uid } }
    if (swapExercise != null) {
        ExerciseLibraryPicker(
            exclude = day.exercises.map { it.libId }.toSet(),
            onDismiss = { swapExerciseUid = null },
            onConfirm = { picked ->
                picked.firstOrNull()?.let { onSwapExercise(swapExercise.uid, it) }
                swapExerciseUid = null
            },
            title = "Swap ${swapExercise.name}",
            confirmLabel = "Swap",
            singleSelect = true
        )
    }
}

/** One exercise, drawn openly: equipment glyph + name/muscle, sets × reps as right meta, × to
 *  remove. The whole row opens the sets/reps sheet; a faint wash appears only while dragging. */
@Composable
private fun ExerciseRow(
    exercise: BuilderExercise,
    dragging: Boolean,
    onOpen: () -> Unit,
    onRemove: () -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier.fillMaxWidth()
            // Clip + wash ONLY while dragging — an always-on rounded clip shaves the leading glyph
            // sitting in the corner arc.
            .then(
                if (dragging) Modifier.clip(RoundedCornerShape(12.dp))
                    .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f))
                else Modifier
            )
            .bounceCombinedClick(onClickLabel = "Sets and reps for ${exercise.name}", onClick = onOpen)
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Icon(
            ExerciseIcons.forEquipment(ExerciseLibrary.byId(exercise.libId)?.equipment ?: emptyList()),
            contentDescription = null,
            tint = muted,
            modifier = Modifier.size(20.dp)
        )
        Column(Modifier.weight(1f)) {
            Text(exercise.name, style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onBackground, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(exercise.muscle, style = MaterialTheme.typography.bodySmall, color = muted)
        }
        Text("${exercise.sets} × ${exercise.reps}", style = MaterialTheme.typography.labelSmall,
            color = muted.copy(alpha = 0.7f))
        // Error at full strength (§5); GlyphButton guarantees the ≥48dp touch target.
        GlyphButton("×", "Remove ${exercise.name}", MaterialTheme.colorScheme.error, onClick = onRemove)
    }
}

/** Sets × reps for one exercise: a stepper for sets (§13 — no keyboard for hot-path numbers), rep
 *  presets as pills with a custom fallback, and an in-place swap. Edits apply live; Done just closes. */
@Composable
private fun SetsRepsSheet(
    exercise: BuilderExercise,
    onSet: (Int, String) -> Unit,
    onSwap: () -> Unit,
    onDismiss: () -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val sheetState = rememberModalBottomSheetState()
    var customMode by remember(exercise.uid) { mutableStateOf(exercise.reps !in REP_PRESETS) }
    var customText by remember(exercise.uid) { mutableStateOf(if (exercise.reps in REP_PRESETS) "" else exercise.reps) }

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheetState) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Column {
                Text(exercise.name, style = MaterialTheme.typography.headlineSmall, color = onBg)
                Spacer(Modifier.height(8.dp))
                Text(exercise.muscle.uppercase(), style = MaterialTheme.typography.labelMedium,
                    color = muted, letterSpacing = 1.sp)
            }
            Column {
                Text("SETS", style = MaterialTheme.typography.labelMedium, color = muted, letterSpacing = 1.sp)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    GlyphButton(
                        "−", "Fewer sets", onBg,
                        enabled = exercise.sets > 1,
                        style = MaterialTheme.typography.titleLarge,
                        onClick = { onSet(exercise.sets - 1, exercise.reps) }
                    )
                    Box(Modifier.width(56.dp), contentAlignment = Alignment.Center) {
                        Text("${exercise.sets}", style = MaterialTheme.typography.headlineMedium, color = onBg)
                    }
                    GlyphButton(
                        "+", "More sets", onBg,
                        enabled = exercise.sets < 20,
                        style = MaterialTheme.typography.titleLarge,
                        onClick = { onSet(exercise.sets + 1, exercise.reps) }
                    )
                }
            }
            Column {
                Text("REPS", style = MaterialTheme.typography.labelMedium, color = muted, letterSpacing = 1.sp)
                Spacer(Modifier.height(10.dp))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    REP_PRESETS.forEach { preset ->
                        SegmentPill(
                            text = preset,
                            selected = !customMode && exercise.reps == preset,
                            onClick = { customMode = false; onSet(exercise.sets, preset) },
                            accent = MaterialTheme.colorScheme.primary,
                            onBg = onBg, muted = muted, outline = MaterialTheme.colorScheme.outline
                        )
                    }
                    SegmentPill(
                        text = "Custom",
                        selected = customMode,
                        onClick = { customMode = true },
                        accent = MaterialTheme.colorScheme.primary,
                        onBg = onBg, muted = muted, outline = MaterialTheme.colorScheme.outline
                    )
                }
                if (customMode) {
                    Spacer(Modifier.height(10.dp))
                    OutlinedTextField(
                        value = customText,
                        onValueChange = { v ->
                            customText = v.take(12)
                            customText.trim().ifBlank { null }?.let { onSet(exercise.sets, it) }
                        },
                        singleLine = true,
                        placeholder = { Text("e.g. 10/leg") },
                        modifier = Modifier.fillMaxWidth()
                    )
                }
            }
            ForgeOutlineCapsule("Swap exercise", onClick = onSwap, modifier = Modifier.fillMaxWidth())
            ForgePrimaryCapsule("Done", onClick = onDismiss, modifier = Modifier.fillMaxWidth())
        }
    }
}
