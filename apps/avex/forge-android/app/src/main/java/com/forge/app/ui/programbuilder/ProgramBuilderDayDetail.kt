@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class, androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.programbuilder

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Edit
import com.forge.app.ui.common.window.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import com.forge.app.ui.common.window.ModalBottomSheet
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.program.ExerciseLibrary
import com.forge.app.ui.common.Corners
import com.forge.app.ui.common.DraggableItem
import com.forge.app.ui.common.ExerciseIcons
import com.forge.app.ui.common.ExerciseLibraryPicker
import com.forge.app.ui.common.ForgeChromeIconButton
import com.forge.app.ui.common.ForgeFieldRow
import com.forge.app.ui.common.ForgeGlyphBadge
import com.forge.app.ui.common.ForgeGroupCaption
import com.forge.app.ui.common.ForgeGroupLabel
import com.forge.app.ui.common.ForgeGroupSection
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeRowGroup
import com.forge.app.ui.common.ForgeSecondaryCapsule
import com.forge.app.ui.common.ForgeTileGrid
import com.forge.app.ui.common.ForgeTopBar
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.GlyphButton
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.dragContainer
import com.forge.app.ui.common.memberFill
import com.forge.app.ui.common.memberShape
import com.forge.app.ui.common.parseAccentHex
import com.forge.app.ui.common.rememberDragDropState
import com.forge.app.ui.common.rowShape

/**
 * One day of the plan, editable: rename (the serif name itself), type + colour, and the exercise
 * list — tap a row for its sets × reps sheet (steppers + rep presets + in-place swap), long-press-drag
 * to reorder, the remove capsule to remove (undone via the shared snackbar). Day-level one-shots (add / duplicate /
 * remove) group at the page end (§3).
 *
 * Which dialog/sheet is open is hoisted ([dialog] / [onDialog]) rather than `remember`ed here, so it
 * is saved with the draft and comes back after rotation or a process kill (H-13). The values typed
 * inside a dialog are `rememberSaveable`, keyed on the row they belong to.
 */
@Composable
fun ProgramBuilderDayDetail(
    day: BuilderDay,
    snackbarHostState: SnackbarHostState,
    dialog: DayDialog,
    onDialog: (DayDialog) -> Unit,
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
    fun closeDialog() = onDialog(DayDialog.None)

    // System back steps out of the day, not out of the whole builder.
    BackHandler { onBack() }

    val listState = rememberLazyListState()
    // Four leading items (name, TYPE, COLOR, EXERCISES anchor) sit above the draggable rows.
    val dragState = rememberDragDropState(listState, firstDraggableIndex = 4) { from, to -> onMoveExercise(from, to) }

    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground

    Scaffold(
        // §4.6: back alone, never the screen's name — the day name heads the content.
        topBar = { ForgeTopBar(onBack = onBack) },
        snackbarHost = { SnackbarHost(snackbarHostState) },
        bottomBar = {
            // §3: this page's one-shots at the END — add (do-it-now) + duplicate / remove sidekicks.
            Column(
                Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                ForgePrimaryCapsule("+ Add exercise", onClick = { onDialog(DayDialog.AddExercises) }, modifier = Modifier.fillMaxWidth())
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    ForgeSecondaryCapsule("Duplicate day", onClick = onDuplicateDay, modifier = Modifier.weight(1f))
                    ForgeSecondaryCapsule(
                        "Remove day",
                        onClick = onRemoveDay,
                        modifier = Modifier.weight(1f),
                        destructive = true
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
            // The seam of the exercise group; the leading items carry their own air above them.
            verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)
        ) {
            item(key = "name") {
                // One affordance: the name itself renames; the drawn pencil capsule makes it
                // discoverable without being a second tap target.
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    modifier = Modifier
                        .bounceCombinedClick(onClickLabel = "Rename day", onClick = { onDialog(DayDialog.Rename) })
                        .padding(vertical = 4.dp)
                ) {
                    Text(day.name, style = MaterialTheme.typography.headlineSmall, color = onBg,
                        maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                    Box(
                        Modifier.size(36.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceContainerHigh),
                        contentAlignment = Alignment.Center
                    ) {
                        Icon(Icons.Filled.Edit, contentDescription = null, tint = muted, modifier = Modifier.size(18.dp))
                    }
                }
            }
            item(key = "type") {
                Box(Modifier.padding(top = 20.dp)) {
                    ForgeGroupSection("Type") {
                        // Normalize so a stored "upper-a"/"lower-b" still selects its base type.
                        val current = baseArchetype(day.archetype)
                        ForgeTileGrid(DAY_TYPES, cols = if (LocalDensity.current.fontScale > 1.3f) 2 else 4) { (key, label, _), corners, modifier ->
                            BuilderTextTile(
                                label = label,
                                selected = current == key,
                                corners = corners,
                                onClick = { onSetType(key) },
                                modifier = modifier
                            )
                        }
                    }
                }
            }
            item(key = "color") {
                Column(Modifier.padding(top = 20.dp)) {
                    Box(Modifier.padding(horizontal = 4.dp)) { ForgeGroupLabel("Color") }
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
                Column(
                    Modifier.padding(top = 16.dp, bottom = 10.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    Box(Modifier.padding(horizontal = 4.dp)) { ForgeGroupLabel("Exercises", meta = "${day.totalSets} sets") }
                    Box(Modifier.padding(horizontal = 4.dp)) {
                        if (day.exercises.isEmpty()) {
                            InlineEmptyHint("No exercises yet. Add one below.", muted.copy(alpha = 0.7f))
                        } else {
                            ForgeGroupCaption("Tap for sets and reps. Hold to reorder.")
                        }
                    }
                }
            }
            itemsIndexed(day.exercises, key = { _, e -> e.uid }) { index, e ->
                DraggableItem(dragState, index) { dragging ->
                    ExerciseRow(
                        exercise = e,
                        shape = rowShape(index, day.exercises.size),
                        dragging = dragging,
                        // uid, not a snapshot: the sheet re-reads the live exercise so stepper taps render immediately.
                        onOpen = { onDialog(DayDialog.SetsReps(e.uid)) },
                        onRemove = { onRemoveExercise(e.uid) }
                    )
                }
            }
        }
    }

    if (dialog is DayDialog.Rename) {
        // Saveable, keyed on the day: the half-typed name survives a rotation with the dialog.
        var text by rememberSaveable(day.uid) { mutableStateOf(day.name) }
        val save = { onRename(text.trim().ifBlank { day.name }); closeDialog() }
        AlertDialog(
            onDismissRequest = { closeDialog() },
            containerColor = MaterialTheme.colorScheme.surface,
            title = { Text("Day name") },
            text = {
                ForgeGroupSection(label = null, footer = { ForgeGroupCaption("${text.length}/$MAX_DAY_NAME") }) {
                    ForgeRowGroup({
                        ForgeFieldRow(
                            label = "Name",
                            value = text,
                            onValueChange = { text = it.take(MAX_DAY_NAME) },
                            placeholder = day.name,
                            keyboardOptions = KeyboardOptions(
                                capitalization = KeyboardCapitalization.Words,
                                imeAction = ImeAction.Done
                            ),
                            keyboardActions = KeyboardActions(onDone = { save() })
                        )
                    })
                }
            },
            confirmButton = { ForgePrimaryCapsule("Save", onClick = { save() }) },
            dismissButton = { ForgeSecondaryCapsule("Cancel", onClick = { closeDialog() }) }
        )
    }

    // Re-read the live exercise each composition so stepper/pill edits render as they land; a swap
    // or removal that drops the uid simply closes the sheet.
    val sheetExercise = (dialog as? DayDialog.SetsReps)?.let { d -> day.exercises.firstOrNull { it.uid == d.exerciseUid } }
    if (sheetExercise != null) {
        SetsRepsSheet(
            exercise = sheetExercise,
            onSet = { sets, reps -> onSetExercise(sheetExercise.uid, sets, reps) },
            onSwap = { onDialog(DayDialog.Swap(sheetExercise.uid)) },
            onDismiss = { closeDialog() }
        )
    }

    if (dialog is DayDialog.AddExercises) {
        ExerciseLibraryPicker(
            exclude = day.exercises.map { it.libId }.toSet(),
            onDismiss = { closeDialog() },
            onConfirm = { picked -> onAddExercises(picked); closeDialog() }
        )
    }

    val swapExercise = (dialog as? DayDialog.Swap)?.let { d -> day.exercises.firstOrNull { it.uid == d.exerciseUid } }
    if (swapExercise != null) {
        ExerciseLibraryPicker(
            exclude = day.exercises.map { it.libId }.toSet(),
            onDismiss = { closeDialog() },
            onConfirm = { picked ->
                picked.firstOrNull()?.let { onSwapExercise(swapExercise.uid, it) }
                closeDialog()
            },
            title = "Swap ${swapExercise.name}",
            confirmLabel = "Swap",
            singleSelect = true
        )
    }
}

/** One exercise as a member of the day's filled group: equipment badge + name/muscle, sets × reps
 *  as right meta, a remove capsule. The whole row opens the sets/reps sheet; while dragging the
 *  lifted row takes the raised tone so it reads as picked up. */
@Composable
private fun ExerciseRow(
    exercise: BuilderExercise,
    shape: Shape,
    dragging: Boolean,
    onOpen: () -> Unit,
    onRemove: () -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier.fillMaxWidth()
            .clip(shape)
            .background(
                if (dragging) MaterialTheme.colorScheme.surfaceContainerHighest
                else MaterialTheme.colorScheme.surfaceContainerHigh
            )
            .bounceCombinedClick(onClickLabel = "Sets and reps for ${exercise.name}", onClick = onOpen)
            .padding(start = ROW_H, end = 6.dp, top = 8.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        ForgeGlyphBadge(
            ExerciseIcons.forEquipment(ExerciseLibrary.byId(exercise.libId)?.equipment ?: emptyList()),
            selected = false,
            size = 34.dp
        )
        Column(Modifier.weight(1f)) {
            // Wraps rather than clamps: a long name at 200% keeps every word.
            Text(exercise.name, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onBackground)
            Text(exercise.muscle, style = MaterialTheme.typography.bodySmall, color = muted)
        }
        Text("${exercise.sets} × ${exercise.reps}", style = MaterialTheme.typography.labelSmall,
            color = muted.copy(alpha = 0.7f))
        // Error at full strength (§5); the chrome capsule guarantees the ≥48dp touch target.
        ForgeChromeIconButton(Icons.Filled.Close, "Remove ${exercise.name}", onRemove, tint = MaterialTheme.colorScheme.error)
    }
}

/**
 * A pick-one text tile on a connected grid (day type, rep preset). The picked tile rounds out of
 * the group in the accent, like onboarding's answers.
 */
@Composable
private fun BuilderTextTile(
    label: String,
    selected: Boolean,
    corners: Corners,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val shape = memberShape(corners, selected)
    Text(
        label,
        style = MaterialTheme.typography.bodyMedium,
        color = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
        modifier = modifier
            .memberFill(shape, selected)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected; role = Role.RadioButton }
            .padding(horizontal = 6.dp, vertical = 14.dp)
    )
}

/** Sets × reps for one exercise: a stepper for sets (§13 — no keyboard for hot-path numbers), rep
 *  presets as a tile grid with a custom fallback, and an in-place swap. Edits apply live; Done just
 *  closes. */
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
    val focus = LocalFocusManager.current
    // Saveable, keyed on the row: the Custom toggle and its half-typed reps survive a rotation.
    var customMode by rememberSaveable(exercise.uid) { mutableStateOf(exercise.reps !in REP_PRESETS) }
    var customText by rememberSaveable(exercise.uid) { mutableStateOf(if (exercise.reps in REP_PRESETS) "" else exercise.reps) }

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheetState, containerColor = MaterialTheme.colorScheme.surface) {
        Column(
            Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).imePadding()
                .padding(horizontal = 24.dp).padding(bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp)
        ) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(exercise.name, style = MaterialTheme.typography.headlineSmall, color = onBg)
                Text(exercise.muscle.uppercase(), style = MaterialTheme.typography.labelMedium,
                    color = muted, letterSpacing = 1.sp)
            }
            ForgeRowGroup({
                Row(
                    Modifier.fillMaxWidth().padding(start = ROW_H, end = 6.dp, top = 4.dp, bottom = 4.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Sets", style = MaterialTheme.typography.bodyLarge, color = onBg, modifier = Modifier.weight(1f))
                    GlyphButton(
                        "−", "Fewer sets", onBg,
                        enabled = exercise.sets > 1,
                        style = MaterialTheme.typography.titleLarge,
                        onClick = { onSet(exercise.sets - 1, exercise.reps) }
                    )
                    Box(Modifier.widthIn(min = 48.dp), contentAlignment = Alignment.Center) {
                        Text("${exercise.sets}", style = MaterialTheme.typography.headlineMedium, color = onBg)
                    }
                    GlyphButton(
                        "+", "More sets", onBg,
                        enabled = exercise.sets < 20,
                        style = MaterialTheme.typography.titleLarge,
                        onClick = { onSet(exercise.sets + 1, exercise.reps) }
                    )
                }
            })
            ForgeGroupSection("Reps") {
                Column(verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)) {
                    ForgeTileGrid(REP_PRESETS + CUSTOM_REPS, cols = 3) { preset, corners, modifier ->
                        val isCustom = preset == CUSTOM_REPS
                        BuilderTextTile(
                            label = preset,
                            selected = if (isCustom) customMode else !customMode && exercise.reps == preset,
                            corners = if (isCustom && customMode) corners.copy(bottomEnd = false) else corners,
                            onClick = {
                                if (isCustom) customMode = true
                                else { customMode = false; onSet(exercise.sets, preset) }
                            },
                            modifier = modifier
                        )
                    }
                    if (customMode) {
                        ForgeRowGroup({
                            ForgeFieldRow(
                                label = "Custom",
                                value = customText,
                                onValueChange = { v ->
                                    customText = v.take(12)
                                    customText.trim().ifBlank { null }?.let { onSet(exercise.sets, it) }
                                },
                                placeholder = "e.g. 10/leg",
                                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                                keyboardActions = KeyboardActions(onDone = { focus.clearFocus() })
                            )
                        })
                    }
                }
            }
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                ForgeSecondaryCapsule("Swap exercise", onClick = onSwap, modifier = Modifier.fillMaxWidth())
                ForgePrimaryCapsule("Done", onClick = onDismiss, modifier = Modifier.fillMaxWidth())
            }
        }
    }
}

private const val CUSTOM_REPS = "Custom"
