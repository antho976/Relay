@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class, androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.programbuilder

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import com.forge.app.ui.common.ForgeSnackbar
import com.forge.app.ui.common.window.AlertDialog
import androidx.compose.material3.MaterialTheme
import com.forge.app.ui.common.window.ModalBottomSheet
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.program.ExerciseLibrary
import com.forge.app.ui.common.Corners
import com.forge.app.ui.common.ExerciseLibraryPicker
import com.forge.app.ui.common.ForgeFieldRow
import com.forge.app.ui.common.ForgeGroupCaption
import com.forge.app.ui.common.ForgeGroupSection
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeRowGroup
import com.forge.app.ui.common.ForgeSecondaryCapsule
import com.forge.app.ui.common.ForgeTileGrid
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.GlyphButton
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.memberFill
import com.forge.app.ui.common.memberShape

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
    onToggleWeekday: (Int) -> Unit,
    onClearWeekdays: () -> Unit,
    onAddExercises: (Collection<String>) -> Unit,
    onRemoveExercise: (String) -> Unit,
    onSwapExercise: (String, String) -> Unit,
    onMoveExercise: (Int, Int) -> Unit,
    onSetExercise: (String, Int, String) -> Unit,
    onDuplicateDay: () -> Unit,
    onRemoveDay: () -> Unit,
    /** Another day is pinned to a weekday, so the plan runs by weekday and an unpinned day never comes up. */
    weekdayPlan: Boolean = false
) {
    fun closeDialog() = onDialog(DayDialog.None)

    // System back steps out of the day, not out of the whole builder.
    BackHandler { onBack() }

    Box(Modifier.fillMaxSize()) {
        DayEditorPage(
            day = day,
            weekdayPlan = weekdayPlan,
            onBack = onBack,
            onRename = { onDialog(DayDialog.Rename) },
            onSetType = onSetType,
            onSetAccent = onSetAccent,
            onToggleWeekday = onToggleWeekday,
            onClearWeekdays = onClearWeekdays,
            // uid, not a snapshot: the sheet re-reads the live exercise so stepper taps render at once.
            onOpenExercise = { onDialog(DayDialog.SetsReps(it)) },
            onRemoveExercise = onRemoveExercise,
            onMoveExercise = onMoveExercise,
            onAddExercise = { onDialog(DayDialog.AddExercises) },
            onDuplicateDay = onDuplicateDay,
            onRemoveDay = onRemoveDay
        )
        SnackbarHost(snackbarHostState, Modifier.align(Alignment.BottomCenter).navigationBarsPadding()) { ForgeSnackbar(it) }
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
            singleSelect = true,
            preferMuscle = ExerciseLibrary.byId(swapExercise.libId)?.muscle
        )
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
