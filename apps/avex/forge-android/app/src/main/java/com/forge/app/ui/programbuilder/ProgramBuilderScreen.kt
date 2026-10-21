@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
package com.forge.app.ui.programbuilder

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBarsPadding
import com.forge.app.ui.common.ForgeSnackbar
import com.forge.app.ui.common.window.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.theme.LocalForgeSettings
import kotlinx.coroutines.launch

/**
 * The program screen — viewer and editor in ONE page (GYMAP-28), drawn as the 2026-10-21 redesign's
 * Your program ([ProgramPage]): the week as bars, the day you tap as its lifts with today's loads,
 * and Start. Opened with [startInView] it is read-only until the top-bar pencil; editing turns the
 * same page into the editor (handles to reorder, removes, Edit day / Add a lift / Add day / Save).
 * Nothing persists until Save. Opens blank for build-your-own (from onboarding). Edit day opens
 * [ProgramBuilderDayDetail].
 */
@Composable
fun ProgramBuilderScreen(
    blank: Boolean,
    startInView: Boolean,
    onClose: () -> Unit,
    /** Start a workout on this day key; null hides the action (the editor, or no route for it). */
    onStartDay: ((String) -> Unit)? = null,
    viewModel: ProgramBuilderViewModel = hiltViewModel()
) {
    LaunchedEffect(Unit) { viewModel.loadIfNeeded(blank) }
    // A blank builder has nothing to view; everywhere else the pen decides when editing starts.
    val viewOrigin = startInView && !blank
    var viewing by rememberSaveable { mutableStateOf(viewOrigin) }
    // The open day lives in the ViewModel with the draft (H-13): its uid must match the restored
    // days, and both come back together after a process kill. The two confirms below carry no
    // typed values, so surviving rotation (rememberSaveable) is enough for them.
    var showDiscard by rememberSaveable { mutableStateOf(false) }
    var showFreestyleSwitch by rememberSaveable { mutableStateOf(false) }
    val freestyleMode by viewModel.freestyleMode.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()

    // §13 undo over confirm: destructive removes get a short Undo window instead of a dialog. The
    // previous snackbar is dismissed first: the ViewModel keeps ONE staged inverse, newest wins, so an
    // older snackbar left up would offer an Undo that belongs to a different removal.
    fun removedWithUndo(message: String) {
        scope.launch {
            snackbarHostState.currentSnackbarData?.dismiss()
            val r = snackbarHostState.showSnackbar(message, actionLabel = "Undo", duration = SnackbarDuration.Short)
            if (r == SnackbarResult.ActionPerformed) viewModel.undoRemove()
        }
    }

    // Saving a plan flips a "go with the flow" user to follow-a-plan — confirm that switch first so it's
    // never silent. Plan users save straight through. A pen-edit returns to the viewer it came from.
    fun onSaved() {
        snackbarHostState.currentSnackbarData?.dismiss()
        if (viewOrigin) viewing = true else onClose()
    }
    fun attemptSave() { if (freestyleMode) showFreestyleSwitch = true else viewModel.save { onSaved() } }

    // Back out of a pen-edit lands on the viewer (dirty edits confirm + reload first); the viewer
    // and a straight editor close the screen.
    fun attemptClose() {
        when {
            viewing -> onClose()
            viewModel.dirty -> showDiscard = true
            viewOrigin -> viewing = true
            else -> onClose()
        }
    }

    val editingDay = if (viewing) null else viewModel.openDayUid?.let { viewModel.day(it) }
    if (editingDay != null) {
        ProgramBuilderDayDetail(
            day = editingDay,
            snackbarHostState = snackbarHostState,
            dialog = viewModel.dayDialog,
            onDialog = { viewModel.showDayDialog(it) },
            onBack = { viewModel.closeDay() },
            onRename = { viewModel.renameDay(editingDay.uid, it) },
            onSetType = { viewModel.setDayType(editingDay.uid, it) },
            onSetAccent = { viewModel.setDayAccent(editingDay.uid, it) },
            onToggleWeekday = { viewModel.toggleDayWeekday(editingDay.uid, it) },
            onClearWeekdays = { viewModel.clearDayWeekdays(editingDay.uid) },
            onAddExercises = { viewModel.addExercises(editingDay.uid, it) },
            onRemoveExercise = { exUid ->
                viewModel.removeExercise(editingDay.uid, exUid)
                removedWithUndo("Exercise removed")
            },
            onSwapExercise = { exUid, libId -> viewModel.swapExercise(editingDay.uid, exUid, libId) },
            onMoveExercise = { from, to -> viewModel.moveExercise(editingDay.uid, from, to) },
            onSetExercise = { exUid, sets, reps -> viewModel.setExercise(editingDay.uid, exUid, sets, reps) },
            weekdayPlan = viewModel.days.any { it.uid != editingDay.uid && it.weekdays.isNotEmpty() },
            onDuplicateDay = { viewModel.duplicateDay(editingDay.uid); viewModel.closeDay() },
            onRemoveDay = {
                viewModel.removeDay(editingDay.uid)
                viewModel.closeDay()
                removedWithUndo("Day removed")
            }
        )
        return
    }

    BackHandler { attemptClose() }

    val days = viewModel.days
    // Which day is open, by uid so a reorder keeps the user on the day they were reading. The index
    // is the fallback for when that uid is gone (removed, or a reload minted fresh uids): the
    // neighbour that slid into its place opens instead of jumping back to the first day.
    var pickedUid by rememberSaveable { mutableStateOf<String?>(null) }
    var pickedIndex by rememberSaveable { mutableIntStateOf(0) }
    val index = days.indexOfFirst { it.uid == pickedUid }.takeIf { it >= 0 }
        ?: pickedIndex.coerceIn(0, (days.size - 1).coerceAtLeast(0))
    val openDay = days.getOrNull(index)
    // Reads the live list, not this composition's snapshot: Add day picks the day it just appended.
    fun pick(i: Int) { pickedIndex = i; pickedUid = viewModel.days.getOrNull(i)?.uid }
    // Editing waits on the load: an edit over the empty pre-load list was overwritten by the late
    // result, and Save over it wrote an empty program over the real one.
    val editing = !viewing && viewModel.loadComplete

    Box(Modifier.fillMaxSize()) {
        ProgramPage(
            model = ProgramPageModel(
                days = days,
                selected = index,
                editing = editing,
                canEdit = viewing && viewModel.loadComplete,
                loaded = viewModel.loadComplete,
                meta = programMeta(days, viewModel.block),
                deloadNote = viewModel.deloadWeekRunning,
                coachChanges = viewModel.coachChanges,
                loads = viewModel.loads,
                swaps = viewModel.userSwaps,
                // Home only ever starts the NEXT day; this is the one place to train a different one.
                // Saved days only: an unsaved edit has no session to start.
                canStart = onStartDay != null && !editing && !viewModel.dirty && openDay?.exercises?.isNotEmpty() == true,
                saving = viewModel.saving,
                unit = LocalForgeSettings.current.weightUnit
            ),
            actions = ProgramPageActions(
                back = { attemptClose() },
                edit = { if (viewModel.loadComplete) viewing = false },
                select = { pick(it) },
                moveDay = { from, to -> viewModel.moveDay(from, to) },
                moveLift = { from, to -> openDay?.let { viewModel.moveExercise(it.uid, from, to) } },
                removeLift = { exUid ->
                    openDay?.let {
                        viewModel.removeExercise(it.uid, exUid)
                        removedWithUndo("Exercise removed")
                    }
                },
                openLift = { exUid ->
                    openDay?.let {
                        viewModel.openDay(it.uid)
                        viewModel.showDayDialog(DayDialog.SetsReps(exUid))
                    }
                },
                editDay = { openDay?.let { viewModel.openDay(it.uid) } },
                addLift = {
                    openDay?.let {
                        viewModel.openDay(it.uid)
                        viewModel.showDayDialog(DayDialog.AddExercises)
                    }
                },
                // The new day opens in the week straight away, so the user sees where it went.
                addDay = { viewModel.addDay(); pick(viewModel.days.lastIndex) },
                save = { attemptSave() },
                start = { openDay?.let { d -> onStartDay?.invoke(d.key) } }
            )
        )
        SnackbarHost(snackbarHostState, Modifier.align(Alignment.BottomCenter).navigationBarsPadding()) { ForgeSnackbar(it) }
    }

    // The two confirms read like the app's others: the act named on a filled button, the safe answer
    // plain. Discarding unsaved edits can't be undone, so its button takes the error fill.
    val scheme = MaterialTheme.colorScheme
    if (showDiscard) {
        AlertDialog(
            onDismissRequest = { showDiscard = false },
            containerColor = scheme.surface,
            title = { Text("Discard changes?") },
            text = { Text("Your edits to this plan haven't been saved.") },
            confirmButton = {
                Button(
                    onClick = {
                        showDiscard = false
                        if (viewOrigin) { viewModel.discardEdits(); viewing = true } else onClose()
                    },
                    colors = ButtonDefaults.buttonColors(containerColor = scheme.error, contentColor = scheme.onError)
                ) { Text("Discard edits") }
            },
            dismissButton = { TextButton(onClick = { showDiscard = false }) { Text("Keep editing", color = scheme.onSurface) } }
        )
    }

    if (showFreestyleSwitch) {
        AlertDialog(
            onDismissRequest = { showFreestyleSwitch = false },
            containerColor = scheme.surface,
            title = { Text("Switch to following a plan?") },
            text = {
                Text("Saving this plan turns off Go with the flow and starts you on it. You can switch " +
                    "back to free logging anytime in Settings → Program.")
            },
            confirmButton = {
                Button(onClick = { showFreestyleSwitch = false; viewModel.save { onSaved() } }) {
                    Text("Save & follow")
                }
            },
            dismissButton = { TextButton(onClick = { showFreestyleSwitch = false }) { Text("Cancel", color = scheme.onSurface) } }
        )
    }
}
