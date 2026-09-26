@file:OptIn(
    androidx.compose.material3.ExperimentalMaterial3Api::class,
    androidx.compose.foundation.layout.ExperimentalLayoutApi::class
)
package com.forge.app.ui.gym.freestyle

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.parseHold
import com.forge.app.domain.units.parseToLb
import com.forge.app.domain.units.toStoredWeightText
import com.forge.app.ui.common.DraggableItem
import com.forge.app.ui.common.ForgeHapticType
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.dragContainer
import com.forge.app.ui.common.forgeHaptic
import com.forge.app.ui.common.moved
import com.forge.app.ui.common.rememberDragDropState
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Longest gap for which resuming a draft rewinds the session clock to the draft's original open time
 * (keeping a quick navigate-away duration honest). A draft survives app kills, so one picked up hours
 * or days later would otherwise record the whole away time as a single multi-day "workout"; past this
 * window we start the clock fresh from now instead.
 */
private const val MAX_RESUME_REWIND_MS = 6L * 60 * 60 * 1000

/** How many recently performed moves the empty log lists, and the in-session rail carries. */
private const val START_RECENT_LIMIT = 6
private const val RAIL_RECENT_LIMIT = 8

/**
 * The freestyle ("go with the flow") logger: a workout with no fixed plan, logged set by set as it
 * happens or after the fact. It persists as a normal finished session, so history, stats and PRs all
 * pick it up.
 *
 * The flow is built around one open exercise at a time. Its entry slab starts filled with the set
 * just logged (or last time's numbers), so a straight set is a single tap on Log set; the steppers
 * cover a change of plate or a rep either way without the keyboard. Logged sets sit above the slab
 * as a ledger and reopen in it when tapped. Every other exercise folds to one summary line.
 */
@Composable
fun FreestyleLogScreen(
    onBack: () -> Unit,
    viewModel: FreestyleLogViewModel = hiltViewModel(),
    templateViewModel: FreestyleTemplateViewModel = hiltViewModel(),
    // The same instance the exercise browser resolves (both are scoped to this route), so the recent
    // moves it loads once serve the footer's quick-add chips and the browser's Recent rail alike.
    browserViewModel: ExerciseBrowserViewModel = hiltViewModel()
) {
    val weightUnit by viewModel.weightUnit.collectAsStateWithLifecycle()
    val unitLabel = com.forge.app.domain.units.unitLabel(weightUnit)
    val templates by templateViewModel.templates.collectAsStateWithLifecycle()
    val recentDefs by browserViewModel.recent.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val view = LocalView.current
    val settings = LocalForgeSettings.current
    // The log lives in the ViewModel so a rotation, font-size or theme change mid-workout keeps the
    // open exercise and its half-typed set instead of dropping back to the resume prompt.
    val log = viewModel.log

    var items by log.items
    var activeId by log.activeId
    var entries by log.entries
    var tagsOpenFor by log.tagsOpenFor
    var lastTime by log.lastTime
    var pinnedNotes by log.pinnedNotes
    var stopwatch by log.stopwatch
    var lastLoggedAtMs by log.lastLoggedAtMs
    var showBrowser by log.showBrowser
    var showTemplates by log.showTemplates
    var openedAtMs by log.openedAtMs
    var draftId by log.draftId
    var pendingDraft by log.pendingDraft
    var draftChecked by log.draftChecked
    var leaving by log.leaving
    val nowMs by produceState(System.currentTimeMillis()) {
        while (true) { value = System.currentTimeMillis(); delay(1000) }
    }

    // Keep the screen awake while logging so it doesn't lock mid-set (GYMAP-74, mirrors the live
    // session). Gated on the Session setting (default on); released when the screen leaves.
    DisposableEffect(settings.keepScreenOn) {
        view.keepScreenOn = settings.keepScreenOn
        onDispose { view.keepScreenOn = false }
    }

    // Draft persistence: on open, offer to resume an unsaved log; while editing, autosave (debounced).
    LaunchedEffect(Unit) {
        if (draftChecked) return@LaunchedEffect   // already settled before a configuration change
        viewModel.loadDraft()?.takeIf { draftToItems(it, weightUnit).isNotEmpty() }?.let { pendingDraft = it }
        draftChecked = true
    }
    LaunchedEffect(items, openedAtMs, pendingDraft, draftChecked, leaving) {
        if (leaving || !draftChecked || pendingDraft != null) return@LaunchedEffect
        if (items.isEmpty()) { viewModel.clearDraft(); return@LaunchedEffect }
        delay(600)   // debounce: only persist once a burst of edits settles
        viewModel.saveDraft(draftFrom(items, openedAtMs, weightUnit, draftId))
    }

    // Last time's sets and the pinned cue (#112, GYMAP-49) for every move on the log, loaded once
    // each and kept here rather than in the card so scrolling a card away doesn't drop them.
    LaunchedEffect(items.map { it.libId }) {
        items.map { it.libId }.forEach { id ->
            if (id !in lastTime) lastTime = lastTime + (id to viewModel.lastSets(id))
            if (id !in pinnedNotes) pinnedNotes = pinnedNotes + (id to viewModel.pinnedNote(id))
        }
    }

    LaunchedEffect(recentDefs) {
        recentDefs.map { it.id }.filter { it !in lastTime }.forEach { id ->
            lastTime = lastTime + (id to viewModel.lastSets(id))
        }
    }

    val listState = rememberLazyListState()
    // Opening an exercise brings its slab into view: a move added from the footer lands at the bottom.
    LaunchedEffect(activeId) {
        val idx = items.indexOfFirst { it.libId == activeId }
        if (idx >= 0) listState.animateScrollToItem(idx + 1)
    }

    fun leave() {
        leaving = true
        if (draftChecked && pendingDraft == null && items.isNotEmpty()) {
            viewModel.saveDraft(draftFrom(items, openedAtMs, weightUnit, draftId))
        }
        onBack()
    }

    // System Back leaves the same way the toolbar arrow does, flushing the debounced autosave. The
    // overlays register their own handlers and win while they are up; stated here as well so this
    // does not depend on composition order.
    BackHandler(enabled = !showBrowser && !showTemplates) { leave() }

    fun updateExercise(libId: String, transform: (FsExercise) -> FsExercise) {
        items = items.map { if (it.libId == libId) transform(it) else it }
    }

    fun entryFor(ex: FsExercise): FsEntry =
        entries[ex.libId] ?: FsEntry(ex.seedEntry(lastTime[ex.libId].orEmpty(), weightUnit))

    fun open(libId: String?) {
        // An edit left open on the exercise being folded is abandoned, not carried: coming back to
        // it later should show the next set, not a half-finished edit of an old one.
        activeId?.takeIf { it != libId && entries[it]?.editing != null }?.let { entries = entries - it }
        activeId = libId
        tagsOpenFor = null
    }

    fun addExercises(added: List<FsExercise>) {
        // Uniqueness is enforced here as well as in the browser: `libId` keys the lazy list, so a
        // duplicate is a crash on measure, not a cosmetic repeat.
        val onLog = items.map { it.libId }.toSet()
        val fresh = added.filter { it.libId !in onLog }.distinctBy { it.libId }
        if (fresh.isEmpty()) return
        items = items + fresh
        open(fresh.first().libId)
    }

    /** Seed the log from a past session (GYMAP-48) and restart the clock from now. */
    fun repeatWorkout(sessionId: Long) {
        scope.launch {
            val seeded = templateViewModel.loadTemplate(sessionId).toItems(weightUnit)
            if (seeded.isEmpty()) return@launch
            items = seeded
            entries = emptyMap()
            draftId = java.util.UUID.randomUUID().toString()
            openedAtMs = System.currentTimeMillis()
            showTemplates = false
            open(seeded.last().libId)
        }
    }

    fun logSet(ex: FsExercise) {
        val entry = entryFor(ex)
        val running = stopwatch?.takeIf { it.first == ex.libId }
        val set = if (running != null) {
            entry.set.copy(hold = holdText(((System.currentTimeMillis() - running.second) / 1000).toInt()))
        } else entry.set
        if (!ex.isLogged(set)) return
        if (running != null) stopwatch = null
        val editing = entry.editing
        updateExercise(ex.libId) { e ->
            if (editing != null && editing in e.sets.indices) e.copy(sets = e.sets.mapIndexed { i, s -> if (i == editing) set else s })
            else e.copy(sets = e.sets + set)
        }
        // Dropping the entry lets the slab re-seed from the set just logged, tags cleared.
        entries = entries - ex.libId
        tagsOpenFor = null
        if (editing == null) lastLoggedAtMs = System.currentTimeMillis()
        view.forgeHaptic(ForgeHapticType.SET_LOGGED, settings.hapticStrength)
    }

    fun deleteSet(ex: FsExercise, index: Int) {
        val removed = ex.sets.getOrNull(index) ?: return
        updateExercise(ex.libId) { e -> e.copy(sets = e.sets.filterIndexed { i, _ -> i != index }) }
        entries = entries - ex.libId
        viewModel.offerUndo("Set ${index + 1} removed") {
            updateExercise(ex.libId) { e ->
                e.copy(sets = e.sets.toMutableList().apply { add(index.coerceAtMost(size), removed) })
            }
        }
    }

    fun removeExercise(ex: FsExercise) {
        val index = items.indexOfFirst { it.libId == ex.libId }
        if (index < 0) return
        items = items.filterNot { it.libId == ex.libId }
        entries = entries - ex.libId
        if (stopwatch?.first == ex.libId) stopwatch = null
        if (activeId == ex.libId) open(null)
        viewModel.offerUndo("${ex.name} removed") {
            if (items.none { it.libId == ex.libId }) {
                items = items.toMutableList().apply { add(index.coerceAtMost(size), ex) }
            }
        }
    }

    val totalVolumeLb = items.sumOf { it.volumeLb(weightUnit) }
    val loggedSets = items.sumOf { ex -> ex.sets.count { ex.isLogged(it) } }
    val canSave = loggedSets > 0

    fun save() {
        val payload = items.mapNotNull { ex ->
            val sets = ex.sets.mapNotNull { s ->
                val weightLb = if (ex.bodyweight) null else parseToLb(s.weight, weightUnit)
                // weightText is stored ALWAYS IN LB (see toStoredWeightText). Storing the display
                // text raw meant a kg user's "100" was read back as 100 lb by the watch prefill.
                val storedWeightText = if (ex.bodyweight) "" else toStoredWeightText(s.weight, weightUnit)
                if (ex.timed) {
                    val dur = parseHold(s.hold)?.takeIf { it > 0 } ?: return@mapNotNull null
                    FreestyleSetInput(
                        weightText = storedWeightText, weightLb = weightLb, reps = 0, durationSeconds = dur,
                        setType = s.setType, isAmrap = s.isAmrap, toFailure = s.toFailure, rpe = s.rpe
                    )
                } else {
                    val reps = s.reps.toIntOrNull()?.takeIf { it > 0 } ?: return@mapNotNull null
                    FreestyleSetInput(
                        weightText = storedWeightText, weightLb = weightLb, reps = reps,
                        setType = s.setType, isAmrap = s.isAmrap, toFailure = s.toFailure, rpe = s.rpe
                    )
                }
            }
            if (sets.isEmpty()) null
            // The name travels for every non-library row (it becomes swapped_name, which is all an
            // imported row has); a muscle only for a user-created move, the one kind the registry
            // owns. Sending it for imported ids registered them as Chest (audit 2026-09-26, 02).
            else FreestyleExerciseInput(
                ex.libId, sets, ex.name.takeIf { ex.custom },
                ex.muscle?.code.takeIf { ex.custom && isCustomExerciseId(ex.libId) }
            )
        }
        if (payload.isNotEmpty()) {
            leaving = true   // stop the debounced autosave from re-writing the draft after save clears it
            view.forgeHaptic(ForgeHapticType.PR_OR_FINISH, settings.hapticStrength)
            viewModel.save(payload, openedAtMs, draftId) { onBack() }
        }
    }

    // Long-press a card to drag it into a new order (GYMAP-47). One fixed leading item (the header)
    // sits above the rows; bounding to items.size keeps the footer from becoming a drop target.
    val dragState = rememberDragDropState(listState, firstDraggableIndex = 1, draggableItemCount = items.size) { from, to ->
        items = items.moved(from, to)
    }

    val imeVisible = WindowInsets.isImeVisible
    // Recent moves not already on the log, with last time's top set, shared by the empty page's list
    // and the in-session rail. Their last-time sets are prefetched below, so adding one lands with
    // the slab already filled.
    val recentMoves = recentDefs.filter { d -> items.none { it.libId == d.id } }.mapNotNull { d ->
        val ex = fsExerciseFor(d.id) ?: return@mapNotNull null
        val last = lastTime[d.id].orEmpty()
        FsRecentMove(
            libId = d.id,
            name = d.name,
            muscle = d.muscle,
            lastReading = last.topReading(ex.timed, weightUnit),
            lastWhen = last.maxOfOrNull { it.completedAt }?.let { lastDoneLabel(it, nowMs) }
        )
    }
    val libraryCount = remember { com.forge.app.program.ExerciseLibrary.all.count { !it.curatedOnly } }

    Box(Modifier.fillMaxSize()) {
        Scaffold(
            topBar = {
                TopAppBar(
                    // §4.6: back only, never the screen's name; the header below says where you are.
                    title = {},
                    navigationIcon = {
                        IconButton(onClick = { leave() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
                )
            },
            bottomBar = {
                // Hidden while the keyboard is up: typing a set is not the moment to finish, and the
                // bar would sit on top of the slab being typed into.
                AnimatedVisibility(
                    visible = items.isNotEmpty() && pendingDraft == null && !imeVisible,
                    enter = slideInVertically(ForgeMotion.enterTween()) { it } + fadeIn(ForgeMotion.enterTween()),
                    exit = slideOutVertically(ForgeMotion.exitTween()) { it } + fadeOut(ForgeMotion.exitTween())
                ) {
                    ForgePrimaryCapsule(
                        if (canSave) "Save workout · $loggedSets ${if (loggedSets == 1) "set" else "sets"}" else "Save workout",
                        onClick = { save() },
                        enabled = canSave,
                        modifier = Modifier.fillMaxWidth().navigationBarsPadding().padding(horizontal = 24.dp, vertical = 12.dp)
                    )
                }
            },
            containerColor = Color.Transparent
        ) { inner ->
            LazyColumn(
                state = listState,
                modifier = Modifier.fillMaxSize().padding(inner).imePadding().dragContainer(dragState),
                contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 24.dp)
            ) {
                item(key = "header") {
                    val draft = pendingDraft
                    if (draft != null) {
                        val restored = remember(draft, weightUnit) { draftToItems(draft, weightUnit) }
                        FsResumePrompt(
                            items = restored,
                            unitLabel = unitLabel,
                            weightUnit = weightUnit,
                            onResume = {
                                items = restored
                                // Rewind to the original open time so a quick navigate-away keeps the
                                // duration honest, but only within a sane window.
                                val now = System.currentTimeMillis()
                                draftId = draft.draftId
                                openedAtMs = draft.openedAtMs.takeIf { now - it <= MAX_RESUME_REWIND_MS } ?: now
                                pendingDraft = null
                                open(restored.lastOrNull()?.libId)
                            },
                            onStartFresh = {
                                viewModel.clearDraft()
                                pendingDraft = null
                            }
                        )
                    } else {
                        FsSessionHeader(
                            elapsedMs = nowMs - openedAtMs,
                            restMs = lastLoggedAtMs?.let { nowMs - it },
                            exerciseCount = items.size,
                            setCount = loggedSets,
                            volumeLb = totalVolumeLb,
                            weightUnit = weightUnit
                        )
                    }
                }
                itemsIndexed(items, key = { _, ex -> ex.libId }) { i, ex ->
                    DraggableItem(dragState, i) { dragging ->
                        if (ex.libId == activeId) {
                            val entry = entryFor(ex)
                            FsOpenCard(
                                exercise = ex,
                                entry = entry,
                                lastTime = lastTime[ex.libId].orEmpty(),
                                pinnedNote = pinnedNotes[ex.libId].orEmpty(),
                                unitLabel = unitLabel,
                                weightUnit = weightUnit,
                                dragging = dragging,
                                tagsOpen = tagsOpenFor == ex.libId,
                                stopwatchStartMs = stopwatch?.takeIf { it.first == ex.libId }?.second,
                                nowMs = nowMs,
                                onFold = { open(null) },
                                onRemove = { removeExercise(ex) },
                                onEntryChange = { set -> entries = entries + (ex.libId to entry.copy(set = set)) },
                                onToggleTags = { tagsOpenFor = if (tagsOpenFor == ex.libId) null else ex.libId },
                                onLog = { logSet(ex) },
                                onEditSet = { idx ->
                                    val s = ex.sets[idx]
                                    entries = entries + (ex.libId to FsEntry(s, editing = idx))
                                    tagsOpenFor = if (s.hasTags) ex.libId else null
                                },
                                onCancelEdit = { entries = entries - ex.libId; tagsOpenFor = null },
                                onDeleteSet = { idx -> deleteSet(ex, idx) },
                                onRepeatLastTime = {
                                    val sets = lastTime[ex.libId].orEmpty()
                                        .map { it.toEntrySet(ex, weightUnit) }
                                        .filter { ex.isLogged(it) }
                                    if (sets.isNotEmpty()) {
                                        updateExercise(ex.libId) { it.copy(sets = it.sets + sets) }
                                        entries = entries - ex.libId
                                        view.forgeHaptic(ForgeHapticType.SET_LOGGED, settings.hapticStrength)
                                    }
                                },
                                onUseLastSet = { s -> entries = entries + (ex.libId to entry.copy(set = s.toEntrySet(ex, weightUnit))) },
                                onToggleStopwatch = {
                                    val running = stopwatch?.takeIf { it.first == ex.libId }
                                    if (running != null) {
                                        val held = ((System.currentTimeMillis() - running.second) / 1000).toInt()
                                        entries = entries + (ex.libId to entry.copy(set = entry.set.copy(hold = holdText(held))))
                                        stopwatch = null
                                    } else {
                                        stopwatch = ex.libId to System.currentTimeMillis()
                                    }
                                    view.forgeHaptic(ForgeHapticType.COUNTDOWN_TICK, settings.hapticStrength)
                                }
                            )
                        } else {
                            FsFoldedCard(
                                exercise = ex,
                                unitLabel = unitLabel,
                                weightUnit = weightUnit,
                                dragging = dragging,
                                onOpen = { open(ex.libId) }
                            )
                        }
                    }
                }
                if (pendingDraft == null) {
                    item(key = "footer") {
                        if (items.isEmpty()) {
                            FsStartPage(
                                recent = recentMoves.take(START_RECENT_LIMIT),
                                templates = templates,
                                libraryCount = libraryCount,
                                nowMs = nowMs,
                                onSearch = { showBrowser = true },
                                onAdd = { id -> fsExerciseFor(id)?.let { addExercises(listOf(it)) } },
                                onRepeat = { id -> repeatWorkout(id) },
                                onAllTemplates = { showTemplates = true }
                            )
                        } else {
                            FsAddFooter(
                                recent = recentMoves.take(RAIL_RECENT_LIMIT),
                                onSearch = { showBrowser = true },
                                onAdd = { id -> fsExerciseFor(id)?.let { addExercises(listOf(it)) } }
                            )
                        }
                    }
                }
            }
        }

        if (showTemplates) {
            FreestyleTemplatePicker(
                templates = templates,
                onClose = { showTemplates = false },
                onPick = { sessionId -> repeatWorkout(sessionId) }
            )
        }

        if (showBrowser) {
            ExerciseBrowserScreen(
                exclude = items.map { it.libId }.toSet(),
                onClose = { showBrowser = false },
                onConfirm = { picked ->
                    addExercises(picked.mapNotNull { fsExerciseFor(it) })
                    showBrowser = false
                },
                onCreateCustom = { name, muscle ->
                    val id = customExerciseId(name)
                    // Same name twice resolves to the same id; don't add a second row for it.
                    if (items.none { it.libId == id }) {
                        addExercises(listOf(FsExercise(libId = id, name = name, muscle = muscle, bodyweight = false, custom = true)))
                        // The picked muscle has no home on a logged row: register the move's
                        // identity now so stats/recap/reuse know it, whether or not this log is saved.
                        viewModel.registerCustomExercise(id, name, muscle)
                    }
                    showBrowser = false
                },
                viewModel = browserViewModel
            )
        }
    }
}
