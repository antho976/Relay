@file:OptIn(
    androidx.compose.material3.ExperimentalMaterial3Api::class,
    androidx.compose.foundation.layout.ExperimentalLayoutApi::class
)
package com.forge.app.ui.gym.freestyle

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
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
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.parseToLb
import com.forge.app.domain.units.weightInputValue
import com.forge.app.domain.units.formatHold
import com.forge.app.domain.units.parseHold
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.DraggableItem
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.GlyphButton
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.dragContainer
import com.forge.app.ui.common.moved
import com.forge.app.ui.common.rememberDragDropState
import com.forge.app.ui.common.rirLabel
import com.forge.app.ui.common.rpeLabel
import com.forge.app.ui.gym.stats.components.MuscleFigure
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.util.Locale

private data class FsSet(
    val weight: String = "",
    val reps: String = "",
    val setType: String? = null,       // null | "warmup" | "drop" — the mutually-exclusive shape
    val isAmrap: Boolean = false,
    val toFailure: Boolean = false,
    val rpe: Double? = null,
    /** Raw hold-time text for a timed-hold set (GYMAP-51), e.g. "1:30" or "45"; blank for a rep set. */
    val hold: String = ""
) {
    val hasTags: Boolean get() = setType != null || isAmrap || toFailure || rpe != null

    /** Terse mono badges for a folded set, mirroring the session-detail set-table words (GYMAP-46). */
    fun tagLabels(): List<String> = buildList {
        when (setType) { "warmup" -> add("WARM"); "drop" -> add("DROP") }
        if (isAmrap) add("AMRAP")
        if (toFailure) add("FAIL")
        rpe?.let { add("RPE ${rpeLabel(it)}") }
    }
}
private data class FsExercise(
    val libId: String,
    val name: String,
    val muscle: MuscleGroup,
    val bodyweight: Boolean,
    /** Timed-hold exercise (GYMAP-51) — sets log a held duration (mm:ss) instead of reps. */
    val timed: Boolean = false,
    val sets: List<FsSet> = listOf(FsSet())
)

/** Snapshot the current log into a resumable draft (raw typed text is preserved verbatim). */
private fun draftFrom(items: List<FsExercise>, openedAtMs: Long): FreestyleDraft =
    FreestyleDraft(
        openedAtMs = openedAtMs,
        exercises = items.map { ex ->
            FreestyleDraftExercise(ex.libId, ex.sets.map {
                FreestyleDraftSet(it.weight, it.reps, it.setType, it.isAmrap, it.toFailure, it.rpe, it.hold)
            })
        }
    )

/** Rebuild the in-memory log from a draft, re-deriving name/muscle/bodyweight from the library and
 *  dropping any exercise whose library id no longer exists. */
private fun draftToItems(draft: FreestyleDraft): List<FsExercise> =
    draft.exercises.mapNotNull { de ->
        val def = ExerciseLibrary.byId(de.libId) ?: return@mapNotNull null
        FsExercise(
            libId = def.id,
            name = def.name,
            muscle = def.muscle,
            bodyweight = def.unit == ExerciseUnit.BODYWEIGHT,
            timed = def.timed,
            sets = de.sets.map { FsSet(it.weight, it.reps, it.setType, it.isAmrap, it.toFailure, it.rpe, it.hold) }
                .ifEmpty { listOf(FsSet()) }
        )
    }

/** Map a picked past-session template into the logger's in-memory shape (GYMAP-48): sets pre-filled
 *  from that session and converted to the display unit, name/muscle/bodyweight re-derived from the
 *  library, and any move no longer in the library dropped — mirrors [draftToItems]. */
private fun List<FreestyleTemplateExercise>.toItems(useKg: Boolean): List<FsExercise> =
    mapNotNull { te ->
        val def = ExerciseLibrary.byId(te.libId) ?: return@mapNotNull null
        val bodyweight = def.unit == ExerciseUnit.BODYWEIGHT
        FsExercise(
            libId = def.id,
            name = def.name,
            muscle = def.muscle,
            bodyweight = bodyweight,
            timed = def.timed,
            // A timed template seeds structure only — the hold time is re-entered (templates carry no duration yet).
            sets = te.sets.map { s ->
                FsSet(
                    weight = if (bodyweight) "" else s.weightLb?.let { weightInputValue(it, useKg) } ?: "",
                    reps = s.reps.toString()
                )
            }.ifEmpty { listOf(FsSet()) }
        )
    }

/** Volume of one set in lb (weight × reps); bodyweight moves contribute no external load. */
private fun FsExercise.setVolumeLb(set: FsSet, useKg: Boolean): Double {
    if (bodyweight) return 0.0
    val reps = set.reps.toIntOrNull() ?: return 0.0
    val w = parseToLb(set.weight, useKg) ?: return 0.0
    return w * reps
}

/** Nudge a display-unit weight string by [delta] (kept ≥0, '.'-decimal so it re-parses in any locale). */
private fun stepWeightStr(cur: String, delta: Double): String {
    val next = ((cur.toDoubleOrNull() ?: 0.0) + delta).coerceAtLeast(0.0)
    return if (next % 1.0 == 0.0) next.toInt().toString() else String.format(Locale.US, "%.1f", next)
}

private fun stepRepsStr(cur: String, delta: Int): String =
    ((cur.toIntOrNull() ?: 0) + delta).coerceAtLeast(0).toString()

/** Keep digits + a single colon (mm:ss) for a hold field (GYMAP-51) — a stray second colon would make
 *  [com.forge.app.domain.units.parseHold] reject the value and silently drop the set. Mirrors the
 *  cardio duration field's `sanitizeDuration`. */
private fun sanitizeHoldText(input: String): String {
    val filtered = input.filter { it.isDigit() || it == ':' }
    val firstColon = filtered.indexOf(':')
    val collapsed = if (firstColon < 0) filtered
    else filtered.substring(0, firstColon + 1) + filtered.substring(firstColon + 1).replace(":", "")
    return collapsed.take(5)
}

/** mm:ss, or h:mm:ss past an hour — the running session clock. */
private fun formatElapsed(ms: Long): String {
    val s = ms / 1000
    val h = s / 3600; val m = (s % 3600) / 60; val sec = s % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, sec) else "%d:%02d".format(m, sec)
}

/**
 * Longest gap for which resuming a draft rewinds the session clock to the draft's original open time
 * (keeping a quick navigate-away duration honest). A draft survives app kills, so one picked up hours
 * or days later would otherwise record the whole away time as a single multi-day "workout"; past this
 * window we start the clock fresh from now instead.
 */
private const val MAX_RESUME_REWIND_MS = 6L * 60 * 60 * 1000

/**
 * Dedicated freestyle ("go with the flow") logger: log what you did at the gym after the fact, with
 * no fixed plan. Add exercises from the browser, type or step weight × reps per set, and save — it
 * persists as a normal finished session (history/stats/PRs all pick it up). Live/flow archetype: serif
 * number fields on underlines (not boxed inputs), +/- steppers, a running clock, each move's
 * target-muscle thumbnail, and a "copy last time" panel seeded from your last performance.
 */
@Composable
fun FreestyleLogScreen(
    onBack: () -> Unit,
    viewModel: FreestyleLogViewModel = hiltViewModel(),
    templateViewModel: FreestyleTemplateViewModel = hiltViewModel()
) {
    val useKg by viewModel.useKg.collectAsStateWithLifecycle()
    val unitLabel = if (useKg) "kg" else "lb"
    var items by remember { mutableStateOf<List<FsExercise>>(emptyList()) }
    var showBrowser by remember { mutableStateOf(false) }
    // "Start from a past workout" (GYMAP-48): every finished session as a reusable template. Offered
    // only on the empty logger (below), and seeds the log with that session's exercises + sets.
    val templates by templateViewModel.templates.collectAsStateWithLifecycle()
    var showTemplates by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    // When the logger was opened — becomes the saved session's start so its duration isn't ~0. Mutable
    // so resuming a draft can rewind it to the original open time (bounded by MAX_RESUME_REWIND_MS on
    // resume, so a long app-kill gap doesn't inflate the recorded duration).
    var openedAtMs by remember { mutableStateOf(System.currentTimeMillis()) }
    val elapsedMs by produceState(0L, openedAtMs) {
        while (true) { value = System.currentTimeMillis() - openedAtMs; delay(1000) }
    }

    // Keep the screen awake while freestyle logging so it doesn't lock mid-set (GYMAP-74, mirrors the
    // live session). Gated on the Session setting (default on); released when the screen leaves.
    val view = LocalView.current
    val keepScreenOn = LocalForgeSettings.current.keepScreenOn
    DisposableEffect(keepScreenOn) {
        view.keepScreenOn = keepScreenOn
        onDispose { view.keepScreenOn = false }
    }

    // Draft persistence: on open, offer to resume an unsaved log; while editing, autosave (debounced).
    var pendingDraft by remember { mutableStateOf<FreestyleDraft?>(null) }
    var draftChecked by remember { mutableStateOf(false) }
    // Set the moment we commit to leaving (save or back). Kills the debounced autosave so it can't fire
    // after viewModel.save() has cleared the draft — otherwise a late write would resurrect a resume
    // draft for an already-saved workout (offering to log it twice). A key of the autosave effect so
    // flipping it cancels any in-flight delay.
    var leaving by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        viewModel.loadDraft()?.takeIf { draftToItems(it).isNotEmpty() }?.let { pendingDraft = it }
        draftChecked = true
    }
    LaunchedEffect(items, openedAtMs, pendingDraft, draftChecked, leaving) {
        // Hold off until the resume choice is settled so we never overwrite the draft before offering it,
        // and once we're leaving so a debounced save can't outlive the session-save's draft clear.
        if (leaving || !draftChecked || pendingDraft != null) return@LaunchedEffect
        if (items.isEmpty()) { viewModel.clearDraft(); return@LaunchedEffect }
        delay(600)   // debounce: only persist once a burst of edits settles
        viewModel.saveDraft(draftFrom(items, openedAtMs))
    }

    fun updateExercise(i: Int, transform: (FsExercise) -> FsExercise) {
        items = items.mapIndexed { idx, e -> if (idx == i) transform(e) else e }
    }

    // Flush the latest edits before leaving, so a back within the debounce window still keeps the draft.
    fun leave() {
        leaving = true
        if (draftChecked && pendingDraft == null && items.isNotEmpty()) {
            viewModel.saveDraft(draftFrom(items, openedAtMs))
        }
        onBack()
    }

    val totalVolumeLb = items.sumOf { ex -> ex.sets.sumOf { ex.setVolumeLb(it, useKg) } }
    // A set counts as logged when it has reps (rep set) OR a parseable hold time (timed set, GYMAP-51).
    val loggedSets = items.sumOf { ex ->
        ex.sets.count { if (ex.timed) (parseHold(it.hold) ?: 0) > 0 else (it.reps.toIntOrNull() ?: 0) > 0 }
    }
    val canSave = loggedSets > 0

    fun save() {
        val payload = items.mapNotNull { ex ->
            val sets = ex.sets.mapNotNull { s ->
                val weightLb = if (ex.bodyweight) null else parseToLb(s.weight, useKg)
                if (ex.timed) {
                    // Timed hold: valid when the hold time parses to > 0; reps is 0 and ignored.
                    val dur = parseHold(s.hold)?.takeIf { it > 0 } ?: return@mapNotNull null
                    FreestyleSetInput(
                        weightText = if (ex.bodyweight) "" else s.weight.trim(),
                        weightLb = weightLb,
                        reps = 0,
                        durationSeconds = dur,
                        setType = s.setType,
                        isAmrap = s.isAmrap,
                        toFailure = s.toFailure,
                        rpe = s.rpe
                    )
                } else {
                    val reps = s.reps.toIntOrNull()?.takeIf { it > 0 } ?: return@mapNotNull null
                    FreestyleSetInput(
                        weightText = if (ex.bodyweight) "" else s.weight.trim(),
                        weightLb = weightLb,
                        reps = reps,
                        setType = s.setType,
                        isAmrap = s.isAmrap,
                        toFailure = s.toFailure,
                        rpe = s.rpe
                    )
                }
            }
            if (sets.isEmpty()) null else FreestyleExerciseInput(ex.libId, sets)
        }
        if (payload.isNotEmpty()) {
            leaving = true   // stop the debounced autosave from re-writing the draft after save clears it
            viewModel.save(payload, openedAtMs) { onBack() }
        }
    }

    // Long-press an exercise card to drag it into a new order (GYMAP-47). One fixed leading item (the
    // resume prompt / session header) sits above the rows; bounding to items.size keeps the trailing
    // "+ Add exercise" footer from becoming a drop target.
    val listState = rememberLazyListState()
    val dragState = rememberDragDropState(listState, firstDraggableIndex = 1, draggableItemCount = items.size) { from, to ->
        items = items.moved(from, to)
    }

    Box(Modifier.fillMaxSize()) {
        Scaffold(
            topBar = {
                TopAppBar(
                    // §2: wordmark + back, never the screen's name (live-flow screen, no hero needed).
                    title = { ForgeWordmark() },
                    navigationIcon = {
                        IconButton(onClick = { leave() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
                )
            },
            bottomBar = {
                ForgePrimaryCapsule(
                    "Save workout",
                    onClick = { save() },
                    enabled = canSave,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 16.dp)
                )
            },
            containerColor = Color.Transparent
        ) { inner ->
            LazyColumn(
                state = listState,
                modifier = Modifier.fillMaxSize().padding(inner).dragContainer(dragState),
                contentPadding = PaddingValues(horizontal = 24.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(0.dp)
            ) {
                item {
                    val draft = pendingDraft
                    if (draft != null) {
                        // An unsaved log is waiting — offer resume before anything else (items is empty here).
                        FsResumePrompt(
                            exerciseCount = draftToItems(draft).size,
                            onResume = {
                                items = draftToItems(draft)
                                // Rewind to the original open time so a quick navigate-away keeps the
                                // duration honest — but only within a sane window. Past it (an app-kill
                                // gap of hours/days) the original time is meaningless, so start fresh
                                // rather than record the whole away time as workout duration.
                                val now = System.currentTimeMillis()
                                openedAtMs = draft.openedAtMs.takeIf { now - it <= MAX_RESUME_REWIND_MS } ?: now
                                pendingDraft = null
                            },
                            onStartFresh = {
                                viewModel.clearDraft()
                                pendingDraft = null
                            }
                        )
                    } else {
                        FsSessionHeader(
                            elapsedMs = elapsedMs,
                            exerciseCount = items.size,
                            setCount = loggedSets,
                            totalVolumeLb = totalVolumeLb,
                            useKg = useKg,
                            empty = items.isEmpty()
                        )
                    }
                }
                itemsIndexed(items, key = { _, ex -> ex.libId }) { i, ex ->
                    DraggableItem(dragState, i) { dragging ->
                        FsExerciseCard(
                            exercise = ex,
                            dragging = dragging,
                            unitLabel = unitLabel,
                            useKg = useKg,
                            lastSetsProvider = { id -> viewModel.lastSets(id) },
                            pinnedNoteProvider = { id -> viewModel.pinnedNote(id) },
                            onRemove = { items = items.filterIndexed { idx, _ -> idx != i } },
                            onSetChange = { setIdx, set ->
                                updateExercise(i) { e -> e.copy(sets = e.sets.mapIndexed { si, s -> if (si == setIdx) set else s }) }
                            },
                            onAddSet = { updateExercise(i) { e -> e.copy(sets = e.sets + FsSet()) } },
                            onRemoveSet = { setIdx -> updateExercise(i) { e -> e.copy(sets = e.sets.filterIndexed { si, _ -> si != setIdx }) } },
                            onReplaceSets = { newSets -> updateExercise(i) { e -> e.copy(sets = newSets) } }
                        )
                    }
                }
                if (pendingDraft == null) {
                    item {
                        Spacer(Modifier.height(16.dp))
                        ForgeOutlineCapsule("+ Add exercise", onClick = { showBrowser = true }, modifier = Modifier.fillMaxWidth())
                        // Reuse-a-session shortcut — only on the empty log (seeding over a started log is odd)
                        // and only when there's history to reuse (§12: no dead-end entry into an empty picker).
                        if (items.isEmpty() && templates.isNotEmpty()) {
                            Spacer(Modifier.height(12.dp))
                            Text(
                                "start from a past workout →",
                                style = MaterialTheme.typography.labelLarge,
                                color = MaterialTheme.colorScheme.primary,
                                modifier = Modifier
                                    .clickableLabeled("Start from a past workout") { showTemplates = true }
                                    .padding(vertical = 8.dp)
                            )
                        }
                        Spacer(Modifier.height(8.dp))
                    }
                }
            }
        }

        if (showTemplates) {
            FreestyleTemplatePicker(
                templates = templates,
                onClose = { showTemplates = false },
                onPick = { sessionId ->
                    scope.launch {
                        // Seed the log from the past session and (re)start the clock from now.
                        items = templateViewModel.loadTemplate(sessionId).toItems(useKg)
                        openedAtMs = System.currentTimeMillis()
                        showTemplates = false
                    }
                }
            )
        }

        if (showBrowser) {
            ExerciseBrowserScreen(
                exclude = items.map { it.libId }.toSet(),
                onClose = { showBrowser = false },
                onConfirm = { picked ->
                    val added = picked.mapNotNull { id ->
                        val def = ExerciseLibrary.byId(id) ?: return@mapNotNull null
                        FsExercise(libId = def.id, name = def.name, muscle = def.muscle, bodyweight = def.unit == ExerciseUnit.BODYWEIGHT, timed = def.timed)
                    }
                    items = items + added
                    showBrowser = false
                }
            )
        }
    }
}

/** Running clock eyebrow + a live at-a-glance line (exercises · sets · total volume), or the first-run cue. */
@Composable
private fun FsSessionHeader(
    elapsedMs: Long,
    exerciseCount: Int,
    setCount: Int,
    totalVolumeLb: Double,
    useKg: Boolean,
    empty: Boolean
) {
    val cs = MaterialTheme.colorScheme
    Column {
        Text(
            "FREESTYLE · ${formatElapsed(elapsedMs)}",
            style = MaterialTheme.typography.labelMedium,
            color = cs.onSurfaceVariant,
            letterSpacing = 1.sp
        )
        Spacer(Modifier.height(4.dp))
        if (empty) {
            Text(
                "Log what you did — add the machines and exercises, with your weights and reps.",
                style = MaterialTheme.typography.bodySmall,
                color = cs.onSurfaceVariant
            )
        } else {
            val vol = if (totalVolumeLb > 0) " · ${formatWeight(totalVolumeLb, useKg)} volume" else ""
            Text(
                "$exerciseCount exercise${if (exerciseCount == 1) "" else "s"} · $setCount set${if (setCount == 1) "" else "s"}$vol",
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurface
            )
            // Non-obvious control, so a one-line explainer (§13) — only once there's an order to change.
            if (exerciseCount >= 2) {
                Spacer(Modifier.height(8.dp))
                Text(
                    "Hold an exercise to reorder.",
                    style = MaterialTheme.typography.bodySmall,
                    color = cs.onSurfaceVariant.copy(alpha = 0.7f)
                )
            }
        }
        Spacer(Modifier.height(8.dp))
    }
}

/**
 * The freestyle logger's "Resume workout?" gate. An unsaved log is autosaved as a draft that survives
 * navigate-away and app kills, so on reopening we let the user pick up where they left off or start over.
 */
@Composable
private fun FsResumePrompt(
    exerciseCount: Int,
    onResume: () -> Unit,
    onStartFresh: () -> Unit
) {
    val cs = MaterialTheme.colorScheme
    Column {
        Text(
            "UNFINISHED LOG",
            style = MaterialTheme.typography.labelMedium,
            color = cs.onSurfaceVariant,
            letterSpacing = 1.sp
        )
        Spacer(Modifier.height(4.dp))
        Text(
            "$exerciseCount ${if (exerciseCount == 1) "exercise" else "exercises"} from a log you didn't save.",
            style = MaterialTheme.typography.bodyMedium,
            color = cs.onSurface
        )
        Spacer(Modifier.height(16.dp))
        ForgePrimaryCapsule("Resume", onClick = onResume, modifier = Modifier.fillMaxWidth())
        Spacer(Modifier.height(8.dp))
        ForgeOutlineCapsule("Start fresh", onClick = onStartFresh, modifier = Modifier.fillMaxWidth())
    }
}

@Composable
private fun FsExerciseCard(
    exercise: FsExercise,
    dragging: Boolean,
    unitLabel: String,
    useKg: Boolean,
    lastSetsProvider: suspend (String) -> List<LoggedSet>,
    pinnedNoteProvider: suspend (String) -> String,
    onRemove: () -> Unit,
    onSetChange: (Int, FsSet) -> Unit,
    onAddSet: () -> Unit,
    onRemoveSet: (Int) -> Unit,
    onReplaceSets: (List<FsSet>) -> Unit
) {
    val cs = MaterialTheme.colorScheme
    val setsDone = exercise.sets.count { (it.reps.toIntOrNull() ?: 0) > 0 }
    val volumeLb = exercise.sets.sumOf { exercise.setVolumeLb(it, useKg) }
    val weightStep = if (useKg) 2.5 else 5.0

    // Last performance for this move — loaded once, powers the "copy last time" panel.
    var lastSets by remember(exercise.libId) { mutableStateOf<List<LoggedSet>>(emptyList()) }
    var showLast by remember(exercise.libId) { mutableStateOf(false) }
    var expanded by remember(exercise.libId) { mutableStateOf(true) }
    // The pinned cue (#112) a user attached to this move, surfaced read-only while logging (GYMAP-49).
    var pinnedNote by remember(exercise.libId) { mutableStateOf("") }
    // Which set (if any) has its tag editor folded open — one at a time keeps the card compact.
    var openSetIdx by remember(exercise.libId) { mutableStateOf<Int?>(null) }
    LaunchedEffect(exercise.libId) {
        lastSets = lastSetsProvider(exercise.libId)
        pinnedNote = pinnedNoteProvider(exercise.libId)
    }

    // Air alone separates exercise blocks (§1: no section hairlines); it's top padding (not a leading
    // Spacer) so the card stays a single child of the drag wrapper's Box and the air sits outside the
    // picked-up wash. While dragging, a faint clipped wash reads the card as lifted (matches the builder).
    Column(
        modifier = Modifier.fillMaxWidth()
            .padding(top = 20.dp)
            .then(
                if (dragging) Modifier.clip(RoundedCornerShape(12.dp)).background(cs.surfaceVariant.copy(alpha = 0.5f))
                else Modifier
            )
    ) {
        // Header: target-muscle thumbnail + name + a mono meta line that stays visible when collapsed —
        // so a long workout can be tidied by folding done exercises down to a one-line summary.
        Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            MuscleFigure(
                muscle = exercise.muscle,
                lit = lerp(cs.primary, cs.onSurface, 0.32f),
                body = cs.onSurfaceVariant.copy(alpha = 0.14f),
                detail = cs.onSurfaceVariant.copy(alpha = 0.3f),
                modifier = Modifier.width(40.dp).height(44.dp)
            )
            Spacer(Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(exercise.name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, color = cs.onSurface)
                val vol = if (volumeLb > 0) " · ${formatWeight(volumeLb, useKg)}" else ""
                Text(
                    "${exercise.muscle.displayName.uppercase()} · $setsDone SET${if (setsDone == 1) "" else "S"}$vol",
                    style = MaterialTheme.typography.labelMedium,
                    color = cs.onSurfaceVariant
                )
            }
            GlyphButton(if (expanded) "▾" else "▸", if (expanded) "Collapse" else "Expand", cs.onSurfaceVariant, onClick = { expanded = !expanded })
            GlyphButton("×", "Remove exercise", cs.onSurfaceVariant, onClick = onRemove)
        }

        // Pinned cue — the coach note kept in the always-visible header (shows folded too), mirroring
        // the structured Train card's quoted-italic aside so the same cue reads the same everywhere.
        if (pinnedNote.isNotBlank()) {
            Spacer(Modifier.height(8.dp))
            Text(
                "\" $pinnedNote \"",
                style = MaterialTheme.typography.bodySmall,
                color = cs.onSurfaceVariant.copy(alpha = 0.6f),
                fontStyle = FontStyle.Italic
            )
        }

      AnimatedVisibility(
          visible = expanded,
          enter = expandVertically(ForgeMotion.enterTween()) + fadeIn(ForgeMotion.enterTween()),
          exit = shrinkVertically(ForgeMotion.exitTween()) + fadeOut(ForgeMotion.exitTween())
      ) {
        Column(modifier = Modifier.fillMaxWidth()) {
        // Copy-last-time — only when there's a prior performance to copy.
        if (lastSets.isNotEmpty()) {
            Spacer(Modifier.height(8.dp))
            Text(
                "copy last time",
                style = MaterialTheme.typography.labelMedium,
                color = cs.primary,
                modifier = Modifier.clickableLabeled("Show last time you did this") { showLast = !showLast }
            )
            AnimatedVisibility(
                visible = showLast,
                enter = expandVertically(ForgeMotion.enterTween()) + fadeIn(ForgeMotion.enterTween()),
                exit = shrinkVertically(ForgeMotion.exitTween()) + fadeOut(ForgeMotion.exitTween())
            ) {
                LastTimePanel(
                    sets = lastSets,
                    useKg = useKg,
                    timed = exercise.timed,
                    onCopy = {
                        onReplaceSets(
                            lastSets.map { s ->
                                FsSet(
                                    weight = if (exercise.timed) "" else s.weightLb?.let { lb -> weightInputValue(lb, useKg) } ?: "",
                                    reps = if (exercise.timed) "" else s.reps.toString(),
                                    // Legacy holds store the seconds in `reps` (null duration column), so
                                    // fall back to that when copying rather than seeding an empty hold.
                                    hold = if (exercise.timed) formatHold(s.durationSeconds ?: s.reps) else ""
                                )
                            }
                        )
                        showLast = false
                    }
                )
            }
        }

        Spacer(Modifier.height(10.dp))
        // Column labels (mono micro-caps), aligned to the set rows below.
        Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
            Text("SET", style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, fontSize = 9.sp, modifier = Modifier.width(24.dp))
            Text(
                if (exercise.bodyweight) "BODYWEIGHT" else "WEIGHT · ${unitLabel.uppercase()}",
                style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, fontSize = 9.sp,
                modifier = Modifier.weight(1f).padding(start = if (exercise.bodyweight) 0.dp else 36.dp)
            )
            Text(if (exercise.timed) "HOLD" else "REPS", style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, fontSize = 9.sp, modifier = Modifier.padding(start = 36.dp))
            Spacer(Modifier.width(72.dp))
        }
        Spacer(Modifier.height(4.dp))

        exercise.sets.forEachIndexed { setIdx, set ->
            val tagsOpen = openSetIdx == setIdx
            Column(modifier = Modifier.fillMaxWidth()) {
                Row(
                    modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Box(modifier = Modifier.width(24.dp)) {
                        Text("%02d".format(setIdx + 1), style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, fontSize = 9.sp)
                    }
                    if (exercise.bodyweight) {
                        Text("BW", style = MaterialTheme.typography.headlineSmall, color = cs.onSurface, modifier = Modifier.weight(1f))
                    } else {
                        StepBtn("−", "Decrease weight") { onSetChange(setIdx, set.copy(weight = stepWeightStr(set.weight, -weightStep))) }
                        FsUnderlineField(
                            value = set.weight,
                            onValueChange = { onSetChange(setIdx, set.copy(weight = it)) },
                            placeholder = "0",
                            keyboardType = KeyboardType.Decimal,
                            modifier = Modifier.weight(1f)
                        )
                        StepBtn("+", "Increase weight") { onSetChange(setIdx, set.copy(weight = stepWeightStr(set.weight, weightStep))) }
                    }
                    Spacer(Modifier.width(6.dp))
                    if (exercise.timed) {
                        // Timed hold (GYMAP-51): a manual mm:ss field (or bare seconds) in place of reps.
                        FsUnderlineField(
                            value = set.hold,
                            onValueChange = { new -> onSetChange(setIdx, set.copy(hold = sanitizeHoldText(new))) },
                            placeholder = "0:00",
                            keyboardType = KeyboardType.Text,
                            modifier = Modifier.width(64.dp)
                        )
                    } else {
                        StepBtn("−", "Decrease reps") { onSetChange(setIdx, set.copy(reps = stepRepsStr(set.reps, -1))) }
                        FsUnderlineField(
                            value = set.reps,
                            onValueChange = { new -> onSetChange(setIdx, set.copy(reps = new.filter { it.isDigit() })) },
                            placeholder = "0",
                            keyboardType = KeyboardType.Number,
                            modifier = Modifier.width(36.dp)
                        )
                        StepBtn("+", "Increase reps") { onSetChange(setIdx, set.copy(reps = stepRepsStr(set.reps, 1))) }
                    }
                    // Set-type tags (warmup/drop/AMRAP/failure/RPE) fold behind this ⋯ toggle so the
                    // weight×reps entry stays the big primary target (§3 live/flow).
                    FsTagToggle { openSetIdx = if (tagsOpen) null else setIdx }
                    // Text glyph over a stock icon (§8); inert + dimmed when it's the only set.
                    GlyphButton(
                        "×", "Remove set", cs.onSurfaceVariant,
                        onClick = {
                            if (openSetIdx == setIdx) openSetIdx = null
                            onRemoveSet(setIdx)
                        },
                        enabled = exercise.sets.size > 1
                    )
                }
                // Folded set with tags → a terse badge line so it reads at a glance without opening each
                // (passive metadata, bare text, §1). The open editor already shows the same state.
                if (!tagsOpen && set.hasTags) {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier.padding(start = 24.dp, bottom = 4.dp)
                    ) {
                        set.tagLabels().forEach {
                            Text(
                                it,
                                style = MaterialTheme.typography.labelSmall,
                                color = cs.onSurfaceVariant.copy(alpha = 0.7f),
                                fontSize = 9.sp,
                                letterSpacing = 0.5.sp
                            )
                        }
                    }
                }
                AnimatedVisibility(
                    visible = tagsOpen,
                    enter = expandVertically(ForgeMotion.enterTween()) + fadeIn(ForgeMotion.enterTween()),
                    exit = shrinkVertically(ForgeMotion.exitTween()) + fadeOut(ForgeMotion.exitTween())
                ) {
                    FsSetTagPicker(
                        set = set,
                        onChange = { onSetChange(setIdx, it) },
                        onClose = { openSetIdx = null }
                    )
                }
            }
        }
        // §11 "+ log" idiom: an accent mono action line.
        Text(
            "+ add set",
            style = MaterialTheme.typography.labelLarge,
            color = cs.primary,
            modifier = Modifier.clickableLabeled("Add set", onClick = onAddSet).padding(vertical = 8.dp)
        )
        }
      }
    }
}

/** The last session's sets as tappable-to-copy chips + a copy CTA that fills this exercise. */
@Composable
private fun LastTimePanel(sets: List<LoggedSet>, useKg: Boolean, timed: Boolean = false, onCopy: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Column(modifier = Modifier.fillMaxWidth().padding(top = 8.dp, bottom = 4.dp)) {
        Text("LAST TIME", style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, fontSize = 9.sp)
        Spacer(Modifier.height(6.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            sets.forEach { s ->
                val w = s.weightLb?.let { formatWeight(it, useKg) } ?: s.weightText.ifBlank { "BW" }
                Text(
                    // Timed holds show their held time; every other set shows "weight × reps". Legacy
                    // holds (e.g. plank logged before the duration column) carry the seconds in `reps`,
                    // so fall back to that rather than rendering a real hold as "0:00".
                    if (timed) formatHold(s.durationSeconds ?: s.reps) else "$w × ${s.reps}",
                    style = MaterialTheme.typography.labelMedium,
                    color = cs.onSurface,
                    modifier = Modifier
                        .border(1.dp, cs.outline.copy(alpha = 0.4f), RoundedCornerShape(50))
                        .padding(horizontal = 10.dp, vertical = 4.dp)
                )
            }
        }
        Spacer(Modifier.height(10.dp))
        ForgeOutlineCapsule("Copy to sets", onClick = onCopy)
    }
}

/** A compact ⋯ tap target (StepBtn-sized) that folds a set's tag editor open/closed. */
@Composable
private fun FsTagToggle(onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Box(
        modifier = Modifier.sizeIn(minWidth = 32.dp, minHeight = 44.dp).clickableLabeled("Set tags", onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Text("⋯", style = MaterialTheme.typography.titleLarge, color = cs.onSurfaceVariant)
    }
}

/**
 * Per-set tag editor (GYMAP-46): the mutually-exclusive shape (normal/warmup/drop), the independent
 * AMRAP + failure flags, and RPE (6→10, each chip read as both RPE and reps-in-reserve). Slides in
 * under the set row — the row's ⋯ toggles it; every edit flows straight back through [onChange].
 */
@Composable
private fun FsSetTagPicker(set: FsSet, onChange: (FsSet) -> Unit, onClose: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    val curRpe = set.rpe
    val rpeOptions = generateSequence(6.0) { it + 0.5 }.takeWhile { it <= 10.0 }.toList()
    Column(modifier = Modifier.fillMaxWidth().padding(start = 24.dp, top = 6.dp, bottom = 10.dp)) {
        Text("TYPE", style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, fontSize = 9.sp)
        Spacer(Modifier.height(6.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            // Shape is single-select: tapping the active one clears back to normal.
            FsTagChip("Normal", selected = set.setType == null) { onChange(set.copy(setType = null)) }
            FsTagChip("Warmup", selected = set.setType == "warmup") {
                onChange(set.copy(setType = if (set.setType == "warmup") null else "warmup"))
            }
            FsTagChip("Drop", selected = set.setType == "drop") {
                onChange(set.copy(setType = if (set.setType == "drop") null else "drop"))
            }
        }
        Spacer(Modifier.height(12.dp))
        Text("FLAGS", style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, fontSize = 9.sp)
        Spacer(Modifier.height(6.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            FsTagChip("AMRAP", selected = set.isAmrap) { onChange(set.copy(isAmrap = !set.isAmrap)) }
            FsTagChip("Failure", selected = set.toFailure) { onChange(set.copy(toFailure = !set.toFailure)) }
        }
        Spacer(Modifier.height(12.dp))
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("EFFORT · RPE / REPS IN RESERVE", style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, fontSize = 9.sp)
            // Clears the RPE when one's set, otherwise just closes the editor — never a stuck-open panel.
            Text(
                if (curRpe != null) "clear" else "done",
                style = MaterialTheme.typography.labelSmall,
                color = cs.onSurfaceVariant.copy(alpha = 0.7f),
                fontSize = 10.sp,
                modifier = Modifier
                    .clickableLabeled(if (curRpe != null) "Clear RPE" else "Close set tags") {
                        if (curRpe != null) onChange(set.copy(rpe = null)) else onClose()
                    }
                    .padding(horizontal = 4.dp, vertical = 2.dp)
            )
        }
        Spacer(Modifier.height(6.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            rpeOptions.forEach { v ->
                val selected = curRpe != null && kotlin.math.abs(curRpe - v) < 0.01
                Column(
                    modifier = Modifier
                        .clip(RoundedCornerShape(6.dp))
                        .then(if (selected) Modifier.background(cs.primaryContainer) else Modifier)
                        .border(1.dp, if (selected) cs.primary else cs.outline.copy(alpha = 0.35f), RoundedCornerShape(6.dp))
                        .clickableLabeled("RPE ${rpeLabel(v)}, ${rirLabel(v)} reps in reserve") { onChange(set.copy(rpe = v)) }
                        .sizeIn(minWidth = 44.dp)
                        .padding(horizontal = 10.dp, vertical = 6.dp),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    Text(rpeLabel(v), style = MaterialTheme.typography.bodyLarge, color = if (selected) cs.onSurface else cs.onSurfaceVariant)
                    Text("${rirLabel(v)} RIR", style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant.copy(alpha = 0.6f), fontSize = 8.sp)
                }
            }
        }
    }
}

/** A pill toggle for one set tag — accent border + wash when on, muted outline when off (§5 ladder). */
@Composable
private fun FsTagChip(label: String, selected: Boolean, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Text(
        label,
        style = MaterialTheme.typography.labelMedium,
        color = if (selected) cs.onSurface else cs.onSurfaceVariant,
        modifier = Modifier
            .clip(RoundedCornerShape(50))
            .then(if (selected) Modifier.background(cs.primaryContainer) else Modifier)
            .border(1.dp, if (selected) cs.primary else cs.outline.copy(alpha = 0.35f), RoundedCornerShape(50))
            .clickableLabeled(label, onClick = onClick)
            .sizeIn(minWidth = 44.dp)
            .padding(horizontal = 14.dp, vertical = 7.dp)
    )
}

/** A compact −/+ tap target (≥44dp) that nudges a set field without opening the keyboard. */
@Composable
private fun StepBtn(symbol: String, label: String, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Box(
        modifier = Modifier.sizeIn(minWidth = 32.dp, minHeight = 44.dp).clickableLabeled(label, onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Text(symbol, style = MaterialTheme.typography.titleLarge, color = cs.onSurfaceVariant)
    }
}

/** A big serif number on an underline — the app's set-entry field (matches the live session), never a boxed input. */
@Composable
private fun FsUnderlineField(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    keyboardType: KeyboardType,
    modifier: Modifier = Modifier
) {
    val cs = MaterialTheme.colorScheme
    Column(modifier = modifier) {
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            textStyle = MaterialTheme.typography.headlineSmall.copy(color = cs.onSurface),
            singleLine = true,
            cursorBrush = SolidColor(cs.primary),
            keyboardOptions = KeyboardOptions(keyboardType = keyboardType),
            decorationBox = { inner ->
                Box {
                    if (value.isEmpty()) {
                        Text(placeholder, style = MaterialTheme.typography.headlineSmall, color = cs.onSurfaceVariant.copy(alpha = 0.35f))
                    }
                    inner()
                }
            }
        )
        HorizontalDivider(modifier = Modifier.padding(top = 2.dp), thickness = 1.dp, color = cs.outline.copy(alpha = 0.5f))
    }
}
