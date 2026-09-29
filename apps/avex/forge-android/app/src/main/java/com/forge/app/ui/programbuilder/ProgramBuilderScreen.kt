@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
package com.forge.app.ui.programbuilder

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Edit
import com.forge.app.ui.common.window.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.program.ExerciseLibrary
import com.forge.app.ui.common.ExerciseIcons
import com.forge.app.ui.common.ForgeChromeIconButton
import com.forge.app.ui.common.ForgeGlyphBadge
import com.forge.app.ui.common.ForgeGroupLabel
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeSecondaryCapsule
import com.forge.app.ui.common.ForgeTopBar
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.rowShape
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.WeekBarRail
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.theme.ForgeMotion
import kotlinx.coroutines.launch

/**
 * The program screen — viewer and editor in ONE layout (GYMAP-28), drawn the way onboarding's
 * "Here's your week" page draws the plan it deals (2026-09-25): serif title, one caption, then the
 * week as one bar per day carrying its sets ([WeekBarRail], the same implementation onboarding uses,
 * so the approved week and the saved one can't read as two different weeks). Tapping a bar swaps
 * the day shown in full underneath: its movements, each with its equipment glyph and sets × reps.
 *
 * Opened with [startInView] it is read-only until the top-bar pencil. As an editor the open day is
 * the tap target into [ProgramBuilderDayDetail], and holding a bar drags it along the week to
 * reorder. Nothing persists until Save. Opens blank for build-your-own (from onboarding).
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
    fun onSaved() {
        // The save consumed any staged removal; don't leave its Undo up over the viewer.
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
    // A one-day week has nothing to move between, so it gets no tap affordance and no line telling
    // the user to use one; its day also drops its set count, since at one day that and the week
    // total are the same fact.
    val many = days.size > 1
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val editing = !viewing && viewModel.loadComplete

    Scaffold(
        topBar = {
            // §4.6: back + ≤1 action — the pencil is the viewer's one action. Inert (dimmed) until
            // the program has loaded, so an edit can never start over an empty list.
            ForgeTopBar(onBack = { attemptClose() }) {
                if (viewing) {
                    ForgeChromeIconButton(
                        Icons.Filled.Edit,
                        "Edit program",
                        { if (viewModel.loadComplete) viewing = false },
                        tint = MaterialTheme.colorScheme.onBackground.copy(alpha = if (viewModel.loadComplete) 1f else 0.35f)
                    )
                }
            }
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
                visible = editing,
                enter = fadeIn(ForgeMotion.enterTween()),
                exit = fadeOut(ForgeMotion.exitTween())
            ) {
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 16.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    ForgeSecondaryCapsule(
                        "+ Add day",
                        // The new day opens in the rail straight away, so the user sees where it went.
                        onClick = { viewModel.addDay(); pick(viewModel.days.lastIndex) },
                        modifier = Modifier.weight(1f)
                    )
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
        Column(
            Modifier
                .fillMaxSize()
                .padding(inner)
                .verticalScroll(rememberScrollState())
                .padding(start = 24.dp, end = 24.dp, top = 8.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(
                "Your program",
                style = MaterialTheme.typography.headlineSmall,
                color = MaterialTheme.colorScheme.onBackground
            )
            val caption = when {
                days.isEmpty() -> null // the hint below carries the empty state
                editing && many -> "Tap the day below to edit it. Hold a bar to reorder."
                editing -> "Tap the day below to edit it."
                many -> "Tap a day to read it."
                else -> null
            }
            if (caption != null) {
                Text(caption, style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.7f))
            }
            // The rows being edited are the deload week's reduced sets, and a save ends its automatic
            // return to full volume, so saving them unchanged makes the recovery week the plan.
            if (editing && viewModel.deloadWeekRunning && days.isNotEmpty()) {
                Text(
                    "You're in a deload week, so these set counts are reduced. Saving keeps them as your plan.",
                    style = MaterialTheme.typography.bodySmall,
                    color = muted
                )
            }
            if (days.isEmpty()) {
                // Honest zero (§12): the plan has no days yet, so there is no week to draw. Not
                // before the load lands, or every open would flash "no days" for a frame.
                if (viewModel.loadComplete) InlineEmptyHint(
                    if (editing) "Add a day to start your plan." else "No days in this plan yet.",
                    muted.copy(alpha = 0.7f)
                )
                return@Column
            }
            Spacer(Modifier.height(6.dp))
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                // The anchor says days and sets because the page title already said "your program".
                Box(Modifier.padding(horizontal = 4.dp)) {
                    ForgeGroupLabel(
                        text = if (many) "${days.size} days" else "1 day",
                        meta = "${days.sumOf { it.totalSets }} sets"
                    )
                }
                WeekBarRail(
                    names = days.map { it.name },
                    sets = days.map { it.totalSets },
                    trackHeight = 104.dp,
                    selectedIndex = index,
                    onSelect = if (many) ({ pick(it) }) else null,
                    keys = days.map { it.uid },
                    onMove = if (editing) ({ from, to -> viewModel.moveDay(from, to) }) else null
                )
            }
            Spacer(Modifier.height(8.dp))
            // Crossfades on both moves the day can make: switching day, and an edit landing in it.
            AnimatedContent(
                targetState = openDay,
                contentKey = { it?.uid },
                transitionSpec = { fadeIn(ForgeMotion.enterTween()) togetherWith fadeOut(ForgeMotion.exitTween()) },
                label = "program_day"
            ) { shown ->
                if (shown != null) {
                    OpenDay(
                        day = shown,
                        showSets = many,
                        onEdit = if (editing) ({ viewModel.openDay(shown.uid) }) else null
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

/**
 * The open day, drawn the way onboarding's week page draws it: mono anchor with its own reading as
 * right meta, then its movements as ONE group of filled rows — equipment badge, name, and sets × reps
 * at the quiet mono rung so the movement names carry the row.
 *
 * In the editor the whole block is ONE tap target into the day editor (never a tap per row), and it
 * closes on a drawn filled `Edit day` capsule so the affordance is visible rather than implied.
 */
@Composable
internal fun OpenDay(day: BuilderDay, showSets: Boolean, onEdit: (() -> Unit)?) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val exercises = day.exercises
    Column(
        Modifier
            .fillMaxWidth()
            .then(
                if (onEdit != null) Modifier.bounceCombinedClick(onClickLabel = "Edit ${day.name}", onClick = onEdit)
                else Modifier
            ),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Box(Modifier.padding(horizontal = 4.dp)) {
            ForgeGroupLabel(
                text = day.name,
                meta = listOfNotNull(
                    day.weekdayLabel(),
                    when {
                        exercises.isEmpty() -> null
                        showSets -> "${exercises.size} moves · ${day.totalSets} sets"
                        else -> "${exercises.size} moves"
                    }
                ).joinToString(" · ").ifEmpty { null }
            )
        }
        if (exercises.isEmpty()) {
            Text(
                "No exercises yet",
                style = MaterialTheme.typography.bodyMedium,
                color = muted,
                modifier = Modifier.padding(horizontal = 4.dp)
            )
        }
        Column(verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)) {
            exercises.forEachIndexed { i, ex ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .clip(rowShape(i, exercises.size))
                        .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                        .padding(horizontal = ROW_H, vertical = 12.dp),
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    ForgeGlyphBadge(
                        ExerciseIcons.forEquipment(ExerciseLibrary.byId(ex.libId)?.equipment.orEmpty()),
                        selected = false,
                        size = 34.dp
                    )
                    Text(
                        ex.name,
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onBackground,
                        modifier = Modifier.weight(1f)
                    )
                    Text("${ex.sets} × ${ex.reps}", style = MaterialTheme.typography.labelMedium, color = muted)
                }
            }
        }
        if (onEdit != null) {
            // Drawn, not separately clickable: the block above is the one tap target (§2③).
            Row(
                Modifier
                    .padding(top = 4.dp)
                    .clip(RoundedCornerShape(50))
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    .padding(horizontal = 18.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Icon(
                    Icons.Filled.Edit,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.size(18.dp)
                )
                Text("Edit day", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground)
            }
        }
    }
}
