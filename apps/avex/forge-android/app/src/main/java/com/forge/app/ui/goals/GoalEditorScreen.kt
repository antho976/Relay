package com.forge.app.ui.goals

import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import com.forge.app.ui.common.ForgeLabelTile
import com.forge.app.ui.common.ForgeTileGrid
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.customPinKey
import com.forge.app.domain.goal.liftPinKey
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.domain.units.distanceInputValue
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.filterDecimalInput
import com.forge.app.domain.units.normalizeDecimalInput
import com.forge.app.domain.units.parseToKm
import com.forge.app.domain.units.parseToLb
import com.forge.app.domain.units.storedUnlessEdited
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.ExerciseIcons
import com.forge.app.ui.common.ForgeChoiceList
import com.forge.app.ui.common.ForgeFieldRow
import com.forge.app.ui.common.ForgeGlyphBadge
import com.forge.app.ui.common.ForgeGroupCaption
import com.forge.app.ui.common.ForgeGroupSection
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeRowGroup
import com.forge.app.ui.common.ForgeSecondaryCapsule
import com.forge.app.ui.common.ForgeSlidingSegments
import com.forge.app.ui.common.ForgeSwitchRow
import com.forge.app.ui.common.ForgeTopBar
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.rowShape
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.filterLibrary
import com.forge.app.ui.theme.LocalForgeSettings

/** Where the editor is in the add/edit flow. Editing an existing goal jumps straight to its form. */
internal sealed interface EditorStep {
    data object ChooseType : EditorStep
    data object LiftPicker : EditorStep
    /** Set/edit a lift's target weight. [currentTargetLb] null when adding. */
    data class LiftWeight(val exerciseId: String, val name: String, val currentTargetLb: Double?) : EditorStep
    data class CustomNew(val metric: GoalMetric) : EditorStep
    data class CustomEdit(val goal: ExtendedGoalRepository.Progress) : EditorStep
}

/**
 * Keeps the add flow's position across configuration change / process death — this is a routed full
 * screen, so plain `remember` would dump the user back to the type chooser on rotation. CustomEdit
 * deliberately saves as nothing: it carries a live [ExtendedGoalRepository.Progress] snapshot, and
 * restoring as null lets the resolve effect re-derive it from the reloaded state.
 */
private val EditorStepSaver = listSaver<EditorStep?, String>(
    save = { step ->
        when (step) {
            null, is EditorStep.CustomEdit -> emptyList()
            EditorStep.ChooseType -> listOf("choose")
            EditorStep.LiftPicker -> listOf("picker")
            is EditorStep.LiftWeight ->
                listOf("lift", step.exerciseId, step.name, step.currentTargetLb?.toString() ?: "")
            is EditorStep.CustomNew -> listOf("new", step.metric.name)
        }
    },
    restore = { saved ->
        when (saved.firstOrNull()) {
            "choose" -> EditorStep.ChooseType
            "picker" -> EditorStep.LiftPicker
            "lift" -> EditorStep.LiftWeight(saved[1], saved[2], saved[3].toDoubleOrNull())
            "new" -> GoalMetric.entries.firstOrNull { it.name == saved.getOrNull(1) }
                ?.let { EditorStep.CustomNew(it) }
            else -> null
        }
    }
)

/**
 * The routed add/edit-goal flow — a full screen pushed from the Goals list, not a dialog. Adding
 * walks type chooser → form (with an exercise picker in between for a lift target); opening with an
 * [exerciseId] or [customId] jumps straight to that goal's form. Back (arrow or system) steps the
 * add flow backwards before leaving the screen.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GoalEditorScreen(
    exerciseId: String?,
    customId: Long?,
    onDone: () -> Unit,
    viewModel: GoalsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val pinned by viewModel.pinnedGoals.collectAsStateWithLifecycle()
    val adding = exerciseId == null && customId == null
    // null while the edit target is still loading; the add flow resolves immediately.
    var step by rememberSaveable(stateSaver = EditorStepSaver) {
        mutableStateOf<EditorStep?>(if (adding) EditorStep.ChooseType else null)
    }
    LaunchedEffect(state.loading) {
        if (step == null && !state.loading) {
            step = when {
                exerciseId != null -> state.liftGoals.firstOrNull { it.exerciseId == exerciseId }
                    ?.let { EditorStep.LiftWeight(it.exerciseId, it.name, it.targetLb) }
                else -> state.customGoals.firstOrNull { it.id == customId }
                    ?.let { EditorStep.CustomEdit(it) }
            }
            if (step == null) onDone() // the goal to edit no longer exists
        }
    }

    fun goBack() {
        when (step) {
            EditorStep.LiftPicker -> step = EditorStep.ChooseType
            is EditorStep.LiftWeight -> if (adding) step = EditorStep.LiftPicker else onDone()
            is EditorStep.CustomNew -> step = EditorStep.ChooseType
            else -> onDone()
        }
    }
    BackHandler { goBack() }

    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val title = when (val s = step) {
        null -> ""
        EditorStep.ChooseType -> "Add a goal"
        EditorStep.LiftPicker -> "Pick an exercise"
        is EditorStep.LiftWeight -> s.name
        is EditorStep.CustomNew -> metricDisplayName(s.metric)
        is EditorStep.CustomEdit -> customGoalTitle(s.goal)
    }

    Scaffold(
        // §4.6: back alone, never the step's name; the content title line below carries it.
        topBar = { ForgeTopBar(onBack = { goBack() }) },
        containerColor = Color.Transparent
    ) { inner ->
        Column(Modifier.fillMaxSize().padding(inner).padding(horizontal = 24.dp)) {
            // The form would otherwise open unlabeled — the step names itself in content.
            if (title.isNotBlank()) {
                Text(title, style = MaterialTheme.typography.headlineSmall, color = onBg)
                Spacer(Modifier.height(20.dp))
            }
            when (val s = step) {
                // No spinners (§13): the edit target resolves instantly from the local DB.
                null -> Box(Modifier.fillMaxSize())
                EditorStep.ChooseType -> ChooseTypeStep(
                    onPickLift = { step = EditorStep.LiftPicker },
                    onPickMetric = { step = EditorStep.CustomNew(it) }
                )
                EditorStep.LiftPicker -> LiftPickerStep(
                    exclude = state.liftPickerExclude,
                    muted = muted,
                    onPick = { id, name -> step = EditorStep.LiftWeight(id, name, null) }
                )
                is EditorStep.LiftWeight -> LiftWeightStep(
                    step = s,
                    pinned = liftPinKey(s.exerciseId) in pinned,
                    onTogglePin = { viewModel.toggleLiftPin(s.exerciseId) },
                    onSet = { lb -> viewModel.setLiftGoal(s.exerciseId, lb); onDone() },
                    onClear = { viewModel.clearLiftGoal(s.exerciseId); onDone() }
                )
                is EditorStep.CustomNew -> CustomNewStep(
                    metric = s.metric,
                    muted = muted,
                    onConfirm = { period, target, label ->
                        viewModel.createCustomGoal(s.metric, period, target, label); onDone()
                    }
                )
                is EditorStep.CustomEdit -> CustomEditStep(
                    goal = s.goal,
                    pinned = customPinKey(s.goal.id) in pinned,
                    onTogglePin = { viewModel.toggleCustomPin(s.goal.id) },
                    onSave = { target -> viewModel.updateCustomGoalTarget(s.goal.id, target); onDone() },
                    onDelete = { viewModel.deleteCustomGoal(s.goal.id); onDone() },
                    onUnchanged = onDone
                )
            }
        }
    }
}

// ─── Steps ──────────────────────────────────────────────────────────────────

/**
 * Step 1 of adding: what kind of goal. Redrawn 2026-09-27 as a two-across grid of tiles, the
 * onboarding gym-setup page's shape: every kind leads with the glyph its goal will carry on every
 * screen afterwards, so the chooser previews its own result, and a short mono line under the name
 * says what the number counts. Strength kinds first, then consistency and body, then cardio. The
 * list it replaced left half the page empty and showed cardio distance and time as twins.
 */
@Composable
internal fun ChooseTypeStep(onPickLift: () -> Unit, onPickMetric: (GoalMetric) -> Unit) {
    val useMiles = LocalForgeSettings.current.useMiles
    val kinds = remember(useMiles) {
        listOf(
            GoalKind(LIFT_KEY, "Lift target", "One weight, one lift", ExerciseIcons.Barbell),
            GoalKind(GoalMetric.VOLUME.name, metricDisplayName(GoalMetric.VOLUME), "Lifted per week", goalGlyph(GoalMetric.VOLUME)),
            GoalKind(GoalMetric.SESSIONS.name, metricDisplayName(GoalMetric.SESSIONS), "Sessions per week", goalGlyph(GoalMetric.SESSIONS)),
            GoalKind(GoalMetric.BODYWEIGHT.name, metricDisplayName(GoalMetric.BODYWEIGHT), "Up or down", goalGlyph(GoalMetric.BODYWEIGHT)),
            GoalKind(GoalMetric.CARDIO_DISTANCE.name, metricDisplayName(GoalMetric.CARDIO_DISTANCE), "${distanceUnitLabel(useMiles)} per week", goalGlyph(GoalMetric.CARDIO_DISTANCE)),
            GoalKind(GoalMetric.CARDIO_MINUTES.name, metricDisplayName(GoalMetric.CARDIO_MINUTES), "Minutes per week", goalGlyph(GoalMetric.CARDIO_MINUTES))
        ).filter { it.key == LIFT_KEY || GoalMetric.valueOf(it.key) in customGoalMetrics }
    }
    Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(bottom = 24.dp)) {
        ForgeGroupCaption("Pick what to aim for. You set the number next.")
        Spacer(Modifier.height(16.dp))
        ForgeTileGrid(kinds, cols = 2) { kind, corners, modifier ->
            ForgeLabelTile(
                label = kind.label,
                meta = kind.meta,
                selected = false,
                corners = corners,
                onClick = {
                    if (kind.key == LIFT_KEY) onPickLift()
                    else GoalMetric.entries.firstOrNull { it.name == kind.key }?.let(onPickMetric)
                },
                modifier = modifier,
                role = Role.Button,
                icon = kind.icon
            )
        }
    }
}

/** One tile of the goal-kind chooser. */
private data class GoalKind(val key: String, val label: String, val meta: String, val icon: ImageVector)

private const val LIFT_KEY = "lift"

private fun metricHint(metric: GoalMetric, useMiles: Boolean): String = when (metric) {
    GoalMetric.CARDIO_DISTANCE -> "e.g. 5 ${distanceUnitLabel(useMiles)} this week, tracked from your cardio"
    GoalMetric.CARDIO_MINUTES -> "e.g. 90 min this week, tracked from your cardio"
    GoalMetric.SESSIONS -> "e.g. train 4 times this week"
    GoalMetric.VOLUME -> "Total lifted this week or month"
    GoalMetric.BODYWEIGHT -> "Reach a target bodyweight, up or down"
}

/**
 * Step 2 for a lift target: a searchable single-select over the whole library. [exclude] drops
 * exercises that already have a goal AND ones you've Hidden in Exercise likes. The results are one
 * group of filled rows, each led by its equipment badge.
 */
@Composable
private fun ColumnScope.LiftPickerStep(exclude: Set<String>, muted: Color, onPick: (id: String, name: String) -> Unit) {
    var query by remember { mutableStateOf("") }
    val results = remember(query, exclude) { filterLibrary(query, exclude) }
    val onBg = MaterialTheme.colorScheme.onBackground
    GoalSearchRow(query, { query = it }, "Search exercises or muscles")
    Spacer(Modifier.height(16.dp))
    LazyColumn(
        Modifier.fillMaxWidth().weight(1f),
        verticalArrangement = Arrangement.spacedBy(GROUP_SEAM),
        contentPadding = PaddingValues(bottom = 24.dp)
    ) {
        itemsIndexed(results, key = { _, def -> def.id }) { i, def ->
            Row(
                Modifier.fillMaxWidth()
                    .clip(rowShape(i, results.size))
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    .clickableLabeled(def.name) { onPick(def.id, def.name) }
                    .padding(horizontal = ROW_H, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp)
            ) {
                // §8: picker rows lead with their equipment-class glyph for wayfinding.
                ForgeGlyphBadge(ExerciseIcons.forEquipment(def.equipment), selected = false, size = 34.dp)
                Column(Modifier.weight(1f)) {
                    Text(def.name, style = MaterialTheme.typography.bodyLarge, color = onBg)
                    Text(def.muscle.displayName, style = MaterialTheme.typography.bodySmall, color = muted)
                }
            }
        }
        if (results.isEmpty()) {
            item {
                Text(
                    "No matches.",
                    style = MaterialTheme.typography.bodyMedium, color = muted,
                    modifier = Modifier.padding(horizontal = 4.dp, vertical = 16.dp)
                )
            }
        }
    }
}

/**
 * The pin control, on both edit forms.
 *
 * Home shows three goals and Antho's call was that the user picks which three. This is where that
 * choice is made — not on the Goals list, where the row is already a single tap target for editing
 * and a second control inside it would be a nested tap in a 48dp row. You are on this screen because
 * you care about this goal, which is exactly when "should it be on Home" is worth asking.
 *
 * A [ForgeSwitchRow] member of the form's group: the ROW owns the tap and announces on/off, the
 * switch is drawn only, so TalkBack never meets a second, unnamed switch.
 */
@Composable
private fun HomePinRow(pinned: Boolean, onToggle: () -> Unit) {
    ForgeSwitchRow(
        label = "Show on Home",
        description = "Home shows three goals at a glance.",
        checked = pinned,
        onToggle = { onToggle() }
    )
}

/** The form's actions at its end: the do-it-now capsule, and a filled destructive sidekick. */
@Composable
private fun EditorActions(
    primary: String,
    enabled: Boolean,
    onPrimary: () -> Unit,
    destructive: String?,
    onDestructive: () -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        ForgePrimaryCapsule(primary, onClick = onPrimary, modifier = Modifier.fillMaxWidth(), enabled = enabled)
        if (destructive != null) {
            // §8's destructive treatment: the filled sidekick with `error` text, never a filled red
            // button. Clearing or deleting is undoable (set it again / the list's Undo).
            ForgeSecondaryCapsule(destructive, onClick = onDestructive, modifier = Modifier.fillMaxWidth(), destructive = true)
        }
    }
}

/** The weight-target form for a lift goal (add or edit). */
@Composable
internal fun LiftWeightStep(
    step: EditorStep.LiftWeight,
    pinned: Boolean,
    onTogglePin: () -> Unit,
    onSet: (Double) -> Unit,
    onClear: () -> Unit
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    // Keyed on weightUnit (like BodyweightLogSheet) so a unit flip re-seeds in the new unit instead of
    // parsing the old unit's digits as the new unit; saveable so a typed target survives rotation.
    val seed = step.currentTargetLb?.let { weightInputValue(it, weightUnit) }
    var weightText by rememberSaveable(step, weightUnit) { mutableStateOf(seed ?: "") }
    // An untouched field saves the stored target: its seed is rounded to 0.1 in the display unit,
    // so 225 lb came back as "102.1" kg, saved as 225.09 lb, and a reached goal read unreached
    // (audit 2026-09-26, 06). Same rule CustomEditStep applies.
    val weightLb = storedUnlessEdited(weightText, seed, step.currentTargetLb) { parseToLb(it, weightUnit) }
    val canSet = weightLb != null && weightLb > 0
    val focus = LocalFocusManager.current
    Column(
        Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(24.dp)
    ) {
        val targetRow: @Composable () -> Unit = {
            ForgeFieldRow(
                label = "Target",
                value = weightText,
                // filterDecimalInput keeps a comma-keyboard's "82,5" as 82.5; dropping the comma made
                // it an 825 kg target (audit 2026-09-26, 06).
                onValueChange = { weightText = filterDecimalInput(it) },
                placeholder = "0",
                suffix = unitLabel(weightUnit),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { focus.clearFocus() })
            )
        }
        // Only for a goal that already exists: pinning something not yet created has nothing to pin.
        if (step.currentTargetLb != null) {
            ForgeRowGroup(targetRow, { HomePinRow(pinned = pinned, onToggle = onTogglePin) })
        } else {
            ForgeRowGroup(targetRow)
        }
        EditorActions(
            primary = "Set goal",
            enabled = canSet,
            onPrimary = { weightLb?.let(onSet) },
            destructive = if (step.currentTargetLb != null) "Clear goal" else null,
            onDestructive = onClear
        )
    }
}

/**
 * The form for a new custom metric goal: enter a target (in the display unit), pick a period for
 * cumulative metrics, optionally name it. [onConfirm] receives the target already converted to the
 * canonical unit (km / minutes / sessions / lb).
 */
@Composable
internal fun CustomNewStep(
    metric: GoalMetric,
    muted: Color,
    onConfirm: (period: GoalPeriod, targetCanonical: Double, label: String) -> Unit
) {
    val settings = LocalForgeSettings.current
    // Saveable (routed full screen): typed input and the picked period survive rotation.
    var valueText by rememberSaveable(metric) { mutableStateOf("") }
    var name by rememberSaveable(metric) { mutableStateOf("") }
    var period by rememberSaveable(metric) {
        mutableStateOf(if (metric == GoalMetric.BODYWEIGHT) GoalPeriod.ALL else GoalPeriod.WEEK)
    }
    val target = parseCustomTarget(metric, valueText, settings.weightUnit, settings.useMiles)
    val nameFocus = remember { FocusRequester() }
    val focus = LocalFocusManager.current

    Column(
        Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(24.dp)
    ) {
        ForgeGroupSection(
            label = null,
            footer = { ForgeGroupCaption(metricHint(metric, settings.useMiles)) }
        ) {
            val rows = buildList<@Composable () -> Unit> {
                add {
                    CustomTargetRow(
                        metric, valueText, { valueText = it },
                        imeAction = ImeAction.Next,
                        onIme = { nameFocus.requestFocus() }
                    )
                }
                if (metric.isCumulative) {
                    add {
                        // Stacked, label over a full-width control: beside a three-way control the
                        // label had too little room and broke mid-word.
                        Column(
                            Modifier.fillMaxWidth().padding(horizontal = ROW_H, vertical = 14.dp),
                            verticalArrangement = Arrangement.spacedBy(10.dp)
                        ) {
                            Text("Timeframe", style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                            ForgeSlidingSegments(
                                options = GoalPeriod.entries.map(::periodLabel),
                                selectedIndex = GoalPeriod.entries.indexOf(period),
                                onSelect = { period = GoalPeriod.entries[it] },
                                modifier = Modifier.fillMaxWidth()
                            )
                        }
                    }
                }
                add {
                    ForgeFieldRow(
                        label = "Name",
                        value = name,
                        onValueChange = { name = it },
                        placeholder = "Optional",
                        keyboardOptions = KeyboardOptions(
                            capitalization = KeyboardCapitalization.Sentences,
                            imeAction = ImeAction.Done
                        ),
                        keyboardActions = KeyboardActions(onDone = { focus.clearFocus() }),
                        focusRequester = nameFocus
                    )
                }
            }
            ForgeRowGroup(*rows.toTypedArray())
        }
        EditorActions(
            primary = "Add goal",
            enabled = target != null && target > 0,
            onPrimary = { target?.let { onConfirm(period, it, name.trim()) } },
            destructive = null,
            onDestructive = {}
        )
    }
}

/** Edit an existing custom goal: change its target or delete it. */
@Composable
private fun CustomEditStep(
    goal: ExtendedGoalRepository.Progress,
    pinned: Boolean,
    onTogglePin: () -> Unit,
    onSave: (targetCanonical: Double) -> Unit,
    onDelete: () -> Unit,
    onUnchanged: () -> Unit
) {
    val settings = LocalForgeSettings.current
    // Seed and buffer are keyed on the display units too: a unit flip while this form is composed
    // re-seeds both in the new unit, so the `changed` comparison below never crosses unit regimes
    // (which would either save a mis-parsed canonical value or silently skip a real save).
    val initial = remember(goal, settings.weightUnit, settings.useMiles) {
        customTargetInputValue(goal.metric, goal.targetValue, settings.weightUnit, settings.useMiles)
    }
    var valueText by rememberSaveable(goal, settings.weightUnit, settings.useMiles) { mutableStateOf(initial) }
    val target = parseCustomTarget(goal.metric, valueText, settings.weightUnit, settings.useMiles)
    // Only persist a genuinely changed target: re-saving the untouched, unit-rounded seed would drift
    // the stored canonical value (e.g. 100 lb shown as "45.4" kg parses back to 100.09 lb).
    val changed = valueText.trim() != initial.trim()
    val focus = LocalFocusManager.current
    Column(
        Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(24.dp)
    ) {
        ForgeRowGroup(
            {
                CustomTargetRow(
                    goal.metric, valueText, { valueText = it },
                    imeAction = ImeAction.Done,
                    onIme = { focus.clearFocus() }
                )
            },
            { HomePinRow(pinned = pinned, onToggle = onTogglePin) }
        )
        // §13 undo over confirm: delete now (the editor pops), with a short Undo in its place —
        // no confirm dialog. What you logged is untouched either way; only the goal itself is
        // removed.
        EditorActions(
            primary = "Save",
            enabled = target != null && target > 0,
            onPrimary = { if (changed) target?.let(onSave) else onUnchanged() },
            destructive = "Delete goal",
            onDestructive = onDelete
        )
    }
}

// ─── Custom-goal unit helpers ─────────────────────────────────────────────

/** Whether a metric's target is a decimal quantity (weights, distance) or a whole count. */
private val GoalMetric.acceptsDecimals: Boolean
    get() = this != GoalMetric.CARDIO_MINUTES && this != GoalMetric.SESSIONS

/** The target-entry row shared by the new-goal and edit-goal forms — one home for the digit
 *  filter, keyboard type and unit so the two forms can't drift apart. */
@Composable
private fun CustomTargetRow(
    metric: GoalMetric,
    valueText: String,
    onValueChange: (String) -> Unit,
    imeAction: ImeAction,
    onIme: () -> Unit
) {
    val settings = LocalForgeSettings.current
    val decimal = metric.acceptsDecimals
    ForgeFieldRow(
        label = "Target",
        value = valueText,
        // A comma-decimal keyboard's "82,5" is kept as 82.5 rather than filtered to 825 (audit
        // 2026-09-26, 06).
        onValueChange = { new -> onValueChange(if (decimal) filterDecimalInput(new) else new.filter { it.isDigit() }) },
        placeholder = "0",
        suffix = customGoalUnitLabel(metric, settings.weightUnit, settings.useMiles),
        keyboardOptions = KeyboardOptions(
            keyboardType = if (decimal) KeyboardType.Decimal else KeyboardType.Number,
            imeAction = imeAction
        ),
        keyboardActions = KeyboardActions(onNext = { onIme() }, onDone = { onIme() })
    )
}

private fun customGoalUnitLabel(metric: GoalMetric, weightUnit: com.forge.app.domain.units.WeightUnit, useMiles: Boolean): String = when (metric) {
    GoalMetric.CARDIO_DISTANCE -> distanceUnitLabel(useMiles)
    GoalMetric.CARDIO_MINUTES -> "min"
    GoalMetric.SESSIONS -> "workouts"
    GoalMetric.VOLUME, GoalMetric.BODYWEIGHT -> unitLabel(weightUnit)
}

/** Parse the target field (display unit) into the metric's canonical unit; null if blank/invalid. */
private fun parseCustomTarget(metric: GoalMetric, text: String, weightUnit: com.forge.app.domain.units.WeightUnit, useMiles: Boolean): Double? =
    when (metric) {
        GoalMetric.CARDIO_DISTANCE -> parseToKm(normalizeDecimalInput(text), useMiles)
        GoalMetric.CARDIO_MINUTES, GoalMetric.SESSIONS -> normalizeDecimalInput(text).toDoubleOrNull()
        GoalMetric.VOLUME, GoalMetric.BODYWEIGHT -> parseToLb(text, weightUnit)
    }

/** Inverse of [parseCustomTarget]: canonical value → bare display-unit string for seeding a field. */
private fun customTargetInputValue(metric: GoalMetric, canonical: Double, weightUnit: com.forge.app.domain.units.WeightUnit, useMiles: Boolean): String =
    when (metric) {
        GoalMetric.CARDIO_DISTANCE -> distanceInputValue(canonical, useMiles)
        GoalMetric.CARDIO_MINUTES, GoalMetric.SESSIONS -> canonical.toInt().toString()
        GoalMetric.VOLUME, GoalMetric.BODYWEIGHT -> weightInputValue(canonical, weightUnit)
    }

// §4: segment labels take ONE short word.
private fun periodLabel(period: GoalPeriod): String = when (period) {
    GoalPeriod.WEEK -> "Week"
    GoalPeriod.MONTH -> "Month"
    GoalPeriod.ALL -> "All"
}
