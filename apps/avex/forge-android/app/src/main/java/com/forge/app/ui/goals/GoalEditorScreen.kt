package com.forge.app.ui.goals

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.domain.units.distanceInputValue
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.parseToKm
import com.forge.app.domain.units.parseToLb
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.ExerciseIcons
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.filterLibrary
import com.forge.app.ui.theme.LocalForgeSettings

/** Where the editor is in the add/edit flow. Editing an existing goal jumps straight to its form. */
private sealed interface EditorStep {
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
        topBar = {
            TopAppBar(
                // §2: wordmark + back, never the step's name — the content title line below carries it.
                title = { ForgeWordmark() },
                navigationIcon = {
                    IconButton(onClick = { goBack() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
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
                    muted = muted,
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
                    onSave = { target -> viewModel.updateCustomGoalTarget(s.goal.id, target); onDone() },
                    onDelete = { viewModel.deleteCustomGoal(s.goal.id); onDone() },
                    onUnchanged = onDone
                )
            }
        }
    }
}

// ─── Steps ──────────────────────────────────────────────────────────────────

/** Step 1 of adding: pick a lift target or one of the custom-goal metrics. */
@Composable
private fun ChooseTypeStep(muted: Color, onPickLift: () -> Unit, onPickMetric: (GoalMetric) -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val useMiles = LocalForgeSettings.current.useMiles
    Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
        GoalTypeOption("Lift target", "Hit a target weight on any exercise", onBg, muted, onPickLift)
        customGoalMetrics.forEach { m ->
            GoalTypeOption(metricDisplayName(m), metricHint(m, useMiles), onBg, muted) { onPickMetric(m) }
        }
    }
}

private fun metricHint(metric: GoalMetric, useMiles: Boolean): String = when (metric) {
    GoalMetric.CARDIO_DISTANCE -> "e.g. 5 ${distanceUnitLabel(useMiles)} this week, tracked from your cardio"
    GoalMetric.CARDIO_MINUTES -> "e.g. 90 min this week, tracked from your cardio"
    GoalMetric.SESSIONS -> "e.g. train 4× this week"
    GoalMetric.VOLUME -> "total lifted this week or month"
    GoalMetric.BODYWEIGHT -> "reach a target bodyweight, up or down"
}

@Composable
private fun GoalTypeOption(title: String, hint: String, onBg: Color, muted: Color, onClick: () -> Unit) {
    Column(Modifier.fillMaxWidth().clickableLabeled(title, onClick = onClick).padding(vertical = 12.dp)) {
        Text(title, style = MaterialTheme.typography.bodyLarge, color = onBg)
        Text(hint, style = MaterialTheme.typography.labelSmall, color = muted)
    }
}

/**
 * Step 2 for a lift target: a searchable single-select over the whole library. [exclude] drops
 * exercises that already have a goal AND ones you've Hidden in Exercise likes.
 */
@Composable
private fun ColumnScope.LiftPickerStep(exclude: Set<String>, muted: Color, onPick: (id: String, name: String) -> Unit) {
    var query by remember { mutableStateOf("") }
    val results = remember(query, exclude) { filterLibrary(query, exclude) }
    val onBg = MaterialTheme.colorScheme.onBackground
    OutlinedTextField(
        value = query,
        onValueChange = { query = it },
        modifier = Modifier.fillMaxWidth(),
        placeholder = { Text("Search exercises or muscles") },
        singleLine = true
    )
    Spacer(Modifier.height(8.dp))
    LazyColumn(Modifier.fillMaxWidth().weight(1f)) {
        items(results, key = { it.id }) { def ->
            Row(
                Modifier.fillMaxWidth()
                    .clickableLabeled(def.name) { onPick(def.id, def.name) }
                    .padding(vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                // §8: picker rows lead with their equipment-class glyph for wayfinding.
                Icon(
                    ExerciseIcons.forEquipment(def.equipment),
                    contentDescription = null,
                    tint = muted,
                    modifier = Modifier.size(20.dp)
                )
                Spacer(Modifier.width(12.dp))
                Column {
                    Text(def.name, style = MaterialTheme.typography.bodyLarge, color = onBg)
                    Text(def.muscle.displayName, style = MaterialTheme.typography.labelSmall, color = muted)
                }
            }
        }
        if (results.isEmpty()) {
            item {
                Text(
                    "No matches.",
                    style = MaterialTheme.typography.bodyMedium, color = muted,
                    modifier = Modifier.padding(vertical = 16.dp)
                )
            }
        }
    }
}

/** The weight-target form for a lift goal (add or edit). */
@Composable
private fun LiftWeightStep(step: EditorStep.LiftWeight, onSet: (Double) -> Unit, onClear: () -> Unit) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    // Keyed on weightUnit (like BodyweightLogSheet) so a unit flip re-seeds in the new unit instead of
    // parsing the old unit's digits as the new unit; saveable so a typed target survives rotation.
    var weightText by rememberSaveable(step, weightUnit) {
        mutableStateOf(step.currentTargetLb?.let { weightInputValue(it, weightUnit) } ?: "")
    }
    val weightLb = parseToLb(weightText, weightUnit)
    Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
        OutlinedTextField(
            value = weightText,
            onValueChange = { weightText = it.filter { c -> c.isDigit() || c == '.' } },
            label = { Text("Target (${unitLabel(weightUnit)})") },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
            modifier = Modifier.fillMaxWidth()
        )
        Spacer(Modifier.height(24.dp))
        ForgePrimaryCapsule(
            "Set goal",
            onClick = { weightLb?.let(onSet) },
            modifier = Modifier.fillMaxWidth(),
            enabled = weightLb != null && weightLb > 0
        )
        if (step.currentTargetLb != null) {
            Spacer(Modifier.height(8.dp))
            // Quiet error-colored text action: clearing a target is reversible (set it again).
            Text(
                "Clear goal",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.clickableLabeled("Clear goal", onClick = onClear).padding(vertical = 8.dp)
            )
        }
    }
}

/**
 * The form for a new custom metric goal: enter a target (in the display unit), pick a period for
 * cumulative metrics, optionally name it. [onConfirm] receives the target already converted to the
 * canonical unit (km / minutes / sessions / lb).
 */
@Composable
private fun CustomNewStep(
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

    Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
        Text(metricHint(metric, settings.useMiles), style = MaterialTheme.typography.bodySmall, color = muted)
        Spacer(Modifier.height(16.dp))
        CustomTargetField(metric, valueText) { valueText = it }
        if (metric.isCumulative) {
            Spacer(Modifier.height(16.dp))
            Text("Timeframe", style = MaterialTheme.typography.labelSmall, color = muted)
            Spacer(Modifier.height(6.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                GoalPeriod.entries.forEach { p ->
                    SegmentPill(
                        text = periodLabel(p),
                        selected = p == period,
                        onClick = { period = p },
                        accent = MaterialTheme.colorScheme.primary,
                        onBg = MaterialTheme.colorScheme.onBackground,
                        muted = muted,
                        outline = MaterialTheme.colorScheme.outline
                    )
                }
            }
        }
        Spacer(Modifier.height(16.dp))
        OutlinedTextField(
            value = name,
            onValueChange = { name = it },
            label = { Text("Name (optional)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        Spacer(Modifier.height(24.dp))
        ForgePrimaryCapsule(
            "Add goal",
            onClick = { target?.let { onConfirm(period, it, name.trim()) } },
            modifier = Modifier.fillMaxWidth(),
            enabled = target != null && target > 0
        )
    }
}

/** Edit an existing custom goal: change its target or delete it. */
@Composable
private fun CustomEditStep(
    goal: ExtendedGoalRepository.Progress,
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
    var confirmDelete by rememberSaveable(goal) { mutableStateOf(false) }
    Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
        CustomTargetField(goal.metric, valueText) { valueText = it }
        Spacer(Modifier.height(24.dp))
        ForgePrimaryCapsule(
            "Save",
            onClick = { if (changed) target?.let(onSave) else onUnchanged() },
            modifier = Modifier.fillMaxWidth(),
            enabled = target != null && target > 0
        )
        Spacer(Modifier.height(8.dp))
        // Quiet error-colored text action; the dialog below words the consequence (§13).
        Text(
            "Delete goal",
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.error,
            modifier = Modifier.clickableLabeled("Delete goal") { confirmDelete = true }.padding(vertical = 8.dp)
        )
    }
    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text("Delete this goal?") },
            text = { Text("The goal and its progress are removed for good. What you logged stays.") },
            confirmButton = {
                TextButton(onClick = { confirmDelete = false; onDelete() }) { Text("Delete") }
            },
            dismissButton = {
                TextButton(onClick = { confirmDelete = false }) { Text("Cancel") }
            }
        )
    }
}

// ─── Custom-goal unit helpers ─────────────────────────────────────────────

/** Whether a metric's target is a decimal quantity (weights, distance) or a whole count. */
private val GoalMetric.acceptsDecimals: Boolean
    get() = this != GoalMetric.CARDIO_MINUTES && this != GoalMetric.SESSIONS

/** The target-entry field shared by the new-goal and edit-goal forms — one home for the digit
 *  filter, keyboard type and unit label so the two forms can't drift apart. */
@Composable
private fun CustomTargetField(metric: GoalMetric, valueText: String, onValueChange: (String) -> Unit) {
    val settings = LocalForgeSettings.current
    val decimal = metric.acceptsDecimals
    OutlinedTextField(
        value = valueText,
        onValueChange = { new -> onValueChange(new.filter { it.isDigit() || (decimal && it == '.') }) },
        label = { Text("Target (${customGoalUnitLabel(metric, settings.weightUnit, settings.useMiles)})") },
        singleLine = true,
        keyboardOptions = KeyboardOptions(
            keyboardType = if (decimal) KeyboardType.Decimal else KeyboardType.Number
        ),
        modifier = Modifier.fillMaxWidth()
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
        GoalMetric.CARDIO_DISTANCE -> parseToKm(text, useMiles)
        GoalMetric.CARDIO_MINUTES, GoalMetric.SESSIONS -> text.trim().toDoubleOrNull()
        GoalMetric.VOLUME, GoalMetric.BODYWEIGHT -> parseToLb(text, weightUnit)
    }

/** Inverse of [parseCustomTarget]: canonical value → bare display-unit string for seeding a field. */
private fun customTargetInputValue(metric: GoalMetric, canonical: Double, weightUnit: com.forge.app.domain.units.WeightUnit, useMiles: Boolean): String =
    when (metric) {
        GoalMetric.CARDIO_DISTANCE -> distanceInputValue(canonical, useMiles)
        GoalMetric.CARDIO_MINUTES, GoalMetric.SESSIONS -> canonical.toInt().toString()
        GoalMetric.VOLUME, GoalMetric.BODYWEIGHT -> weightInputValue(canonical, weightUnit)
    }

// §4: lens/segment pills take ONE short word.
private fun periodLabel(period: GoalPeriod): String = when (period) {
    GoalPeriod.WEEK -> "Week"
    GoalPeriod.MONTH -> "Month"
    GoalPeriod.ALL -> "All"
}
