@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
package com.forge.app.ui.programbuilder

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.DraggableItem
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.dragContainer
import com.forge.app.ui.common.parseAccentHex
import com.forge.app.ui.common.rememberDragDropState
import com.forge.app.ui.theme.ForgeMotion
import kotlinx.coroutines.launch

/**
 * The program screen — viewer and editor in ONE layout (GYMAP-28). The whole plan renders
 * editorially (mono day anchors + set-count meta, an exercise row per slot — the same vocabulary as
 * the onboarding week preview). Opened with [startInView] it is read-only until the top-bar pencil;
 * as an editor, day sections tap into [ProgramBuilderDayDetail] and long-press-drag to reorder.
 * Nothing persists until Save. Opens blank for build-your-own (from onboarding).
 */
@Composable
fun ProgramBuilderScreen(
    blank: Boolean,
    startInView: Boolean,
    onClose: () -> Unit,
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

    // §13 undo over confirm: destructive removes get a short Undo window instead of a dialog.
    //
    // The previous snackbar is dismissed first. The ViewModel keeps ONE staged inverse, newest wins,
    // so leaving an older snackbar up meant offering an Undo whose action belonged to a different
    // removal than its own message: remove A, remove B, tap Undo on A's snackbar, and B came back
    // while A stayed gone.
    fun removedWithUndo(message: String) {
        scope.launch {
            snackbarHostState.currentSnackbarData?.dismiss()
            val r = snackbarHostState.showSnackbar(message, actionLabel = "Undo", duration = SnackbarDuration.Short)
            if (r == SnackbarResult.ActionPerformed) viewModel.undoRemove()
        }
    }

    // Saving a plan flips a "go with the flow" user to follow-a-plan — confirm that switch first so it's
    // never silent. Plan users save straight through. A pen-edit returns to the viewer it came from.
    fun onSaved() { if (viewOrigin) viewing = true else onClose() }
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
            onDialog = { viewModel.updateDayDialog(it) },
            onBack = { viewModel.closeDay() },
            onRename = { viewModel.renameDay(editingDay.uid, it) },
            onSetType = { viewModel.setDayType(editingDay.uid, it) },
            onSetAccent = { viewModel.setDayAccent(editingDay.uid, it) },
            onAddExercises = { viewModel.addExercises(editingDay.uid, it) },
            onRemoveExercise = { exUid ->
                viewModel.removeExercise(editingDay.uid, exUid)
                removedWithUndo("Exercise removed")
            },
            onSwapExercise = { exUid, libId -> viewModel.swapExercise(editingDay.uid, exUid, libId) },
            onMoveExercise = { from, to -> viewModel.moveExercise(editingDay.uid, from, to) },
            onSetExercise = { exUid, sets, reps -> viewModel.setExercise(editingDay.uid, exUid, sets, reps) },
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
    val listState = rememberLazyListState()
    // One leading item (title + summary + caption) sits above the draggable day sections.
    val dragState = rememberDragDropState(listState, firstDraggableIndex = 1) { from, to -> viewModel.moveDay(from, to) }

    Scaffold(
        topBar = {
            TopAppBar(
                // §4.6: bell + back + ≤1 action — the pencil is the viewer's one action.
                title = {},
                navigationIcon = { IconButton(onClick = { attemptClose() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                actions = {
                    if (viewing) {
                        IconButton(
                            onClick = { viewing = false },
                            enabled = viewModel.loadComplete
                        ) { Icon(Icons.Filled.Edit, "Edit program") }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        snackbarHost = { SnackbarHost(snackbarHostState) },
        bottomBar = {
            // §3: the page-level one-shot actions group at the END — one row, not a stacked wall,
            // so edit mode costs the plan as little height as possible.
            AnimatedVisibility(
                // …and not until the program has actually loaded. Editing and Save were live over an
                // EMPTY list while loadDays() was still in flight: an edit made in that window was
                // overwritten wholesale by the late result, and Save over the empty list does not
                // save nothing — it writes an empty program over the real one.
                visible = !viewing && viewModel.loadComplete,
                enter = fadeIn(ForgeMotion.enterTween()),
                exit = fadeOut(ForgeMotion.exitTween())
            ) {
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 16.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    ForgeOutlineCapsule("+ Add day", onClick = { viewModel.addDay() }, modifier = Modifier.weight(1f))
                    ForgePrimaryCapsule(
                        "Save",
                        onClick = { attemptSave() },
                        // Stated here too, not only via the bar's visibility: a Save that fires over
                        // a not-yet-loaded list writes an empty program over the real one.
                        enabled = !viewModel.saving && viewModel.loadComplete,
                        modifier = Modifier.weight(1f)
                    )
                }
            }
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize().padding(inner)
                .then(if (viewing) Modifier else Modifier.dragContainer(dragState)),
            contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 8.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(28.dp)
        ) {
            item(key = "header") {
                ProgramHeader(
                    dayCount = days.size,
                    setCount = days.sumOf { it.totalSets },
                    caption = when {
                        viewing -> null
                        days.isEmpty() -> null // the hint below carries the empty state
                        else -> "Tap a day to edit. Hold to reorder."
                    },
                    emptyHint = if (!viewing && days.isEmpty()) "Add a day to start your plan." else null
                )
            }
            itemsIndexed(days, key = { _, d -> d.uid }) { index, d ->
                DraggableItem(dragState, index) { dragging ->
                    DaySection(
                        day = d,
                        editable = !viewing,
                        dragging = dragging,
                        onOpen = { viewModel.openDay(d.uid) }
                    )
                }
            }
        }
    }

    if (showDiscard) {
        AlertDialog(
            onDismissRequest = { showDiscard = false },
            title = { Text("Discard changes?") },
            text = { Text("Your edits to this plan haven't been saved.") },
            confirmButton = {
                TextButton(onClick = {
                    showDiscard = false
                    if (viewOrigin) { viewModel.discardEdits(); viewing = true } else onClose()
                }) { Text("Discard") }
            },
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
                TextButton(onClick = { showFreestyleSwitch = false; viewModel.save { onSaved() } }) {
                    Text("Save & follow")
                }
            },
            dismissButton = { TextButton(onClick = { showFreestyleSwitch = false }) { Text("Cancel") } }
        )
    }
}

/** Serif page title + the plan's one summary read; honest zeros when the plan is empty (§12). */
@Composable
private fun ProgramHeader(dayCount: Int, setCount: Int, caption: String?, emptyHint: String?) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column {
        Text("Your program", style = MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.onBackground)
        Spacer(Modifier.height(8.dp))
        Text(
            "$dayCount DAYS · $setCount SETS",
            style = MaterialTheme.typography.labelMedium, color = muted, letterSpacing = 1.sp
        )
        if (caption != null) {
            Spacer(Modifier.height(10.dp))
            Text(caption, style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.7f))
        }
        if (emptyHint != null) {
            Spacer(Modifier.height(10.dp))
            InlineEmptyHint(emptyHint, muted.copy(alpha = 0.7f))
        }
    }
}

/**
 * One day, drawn openly (§1): colour-dot + mono name anchor with its set count as right meta, then
 * an exercise row per slot — identical in viewer and editor; the editor only adds tap + drag. The
 * faint wash appears while dragging so the floating section reads as picked up.
 */
@Composable
private fun DaySection(
    day: BuilderDay,
    editable: Boolean,
    dragging: Boolean,
    onOpen: () -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(
        Modifier.fillMaxWidth()
            // Clip + wash ONLY while dragging — an always-on rounded clip shaved the colour dot
            // sitting in the corner arc.
            .then(
                if (dragging) Modifier.clip(RoundedCornerShape(12.dp))
                    .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f))
                else Modifier
            )
            .then(
                if (editable) Modifier.bounceCombinedClick(onClickLabel = "Edit ${day.name}", onClick = onOpen)
                else Modifier
            )
    ) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(Modifier.size(8.dp).clip(CircleShape).background(parseAccentHex(day.accentHex)))
                Text(
                    day.name.uppercase(),
                    // labelLarge 13sp — §7 anchors read as present; the small anchor let sections merge.
                    style = MaterialTheme.typography.labelLarge,
                    color = muted,
                    letterSpacing = 1.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
            Text("${day.totalSets} SETS", style = MaterialTheme.typography.labelSmall, color = muted)
        }
        Spacer(Modifier.height(10.dp))
        day.exercises.forEach { ex ->
            // Hanging indent: rows align under the day NAME (dot 8 + gap 8), so the outdented dot
            // column groups each day the way an outline would — without drawing one. The reps meta
            // sits at the quiet caption rung so the movement names carry the row.
            Row(
                Modifier.fillMaxWidth().padding(start = 16.dp, top = 5.dp, bottom = 5.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    ex.name,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onBackground,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false)
                )
                Spacer(Modifier.width(12.dp))
                Text("${ex.sets} × ${ex.reps}", style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.7f))
            }
        }
    }
}
