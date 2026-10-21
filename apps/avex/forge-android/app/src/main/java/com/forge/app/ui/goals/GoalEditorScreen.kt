package com.forge.app.ui.goals

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.domain.adapt.E1rm
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.domain.goal.HOME_PIN_SLOTS
import com.forge.app.domain.goal.customPinKey
import com.forge.app.domain.goal.liftPinKey
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.fromDisplayDistance
import com.forge.app.domain.units.fromDisplayWeight
import com.forge.app.domain.units.toDisplayDistance
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.trimDecimal
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.ForgeSearchRow
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenRangeSwitch
import com.forge.app.ui.common.OpenSmallButton
import com.forge.app.ui.common.OpenSparkline
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.common.filterLibrary
import com.forge.app.ui.common.toggleableLabeled
import com.forge.app.ui.theme.GoalsType
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import java.time.LocalDate
import java.util.Locale
import kotlin.math.roundToInt

/** Where the editor is in the add/edit flow. Editing an existing goal jumps straight to its target. */
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
 * The routed add/edit-goal flow (2026-10-21 redesign, "pg-goals ga" and "gt"). Adding walks Add a
 * goal (six kinds as quiet tiles) → Set the target, with a lift picker between for a lift target;
 * opening with an [exerciseId] or [customId] jumps straight to that goal's target. Back (arrow or
 * system) steps the add flow backwards before leaving the screen.
 */
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

    // One commit per visit. Each write below is followed by onDone(), and the screen stays tappable
    // through its exit animation: a double tap on Set goal inserted the goal twice (create is an
    // unconditional insert) and popped a second destination along with this one.
    var committed by remember { mutableStateOf(false) }
    fun commit(write: () -> Unit) {
        if (committed) return
        committed = true
        write()
        onDone()
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

    val liveKeys = remember(state.liftGoals, state.customGoals) {
        state.liftGoals.map { liftPinKey(it.exerciseId) }.toSet() + state.customGoals.map { customPinKey(it.id) }
    }
    val livePinned = pinned.filter { it in liveKeys }

    Scaffold(
        topBar = { OpenBackBar(onBack = { goBack() }) },
        containerColor = Color.Transparent
    ) { inner ->
        val body = Modifier.fillMaxSize().padding(inner)
        when (val s = step) {
            // No spinners: the edit target resolves instantly from the local DB.
            null -> Box(body)
            EditorStep.ChooseType -> ChooseTypeStep(
                onPickLift = { step = EditorStep.LiftPicker },
                onPickMetric = { step = EditorStep.CustomNew(it) },
                modifier = body
            )
            EditorStep.LiftPicker -> {
                var trained by remember { mutableStateOf<List<String>>(emptyList()) }
                LaunchedEffect(Unit) { trained = viewModel.recentlyTrainedIds() }
                LiftPickerStep(
                    exclude = state.liftPickerExclude,
                    trainedFirst = trained,
                    onPick = { id, name -> step = EditorStep.LiftWeight(id, name, null) },
                    modifier = body
                )
            }
            is EditorStep.LiftWeight -> {
                val history = produceState<GoalsViewModel.LiftHistory?>(null, s.exerciseId) {
                    value = viewModel.liftHistory(s.exerciseId)
                }.value
                val key = liftPinKey(s.exerciseId)
                LiftTargetStep(
                    step = s,
                    history = history,
                    pinned = key in livePinned,
                    otherPins = livePinned.count { it != key },
                    onSet = { lb, pin -> commit { viewModel.setLiftGoal(s.exerciseId, lb, pin) } },
                    onClear = { commit { viewModel.clearLiftGoal(s.exerciseId) } },
                    modifier = body
                )
            }
            is EditorStep.CustomNew -> CustomTargetStep(
                metric = s.metric,
                goal = null,
                loadHistory = viewModel::customHistory,
                loadWeighIns = viewModel::weighIns90,
                pinned = false,
                otherPins = livePinned.size,
                onCreate = { period, target, label, pin ->
                    commit { viewModel.createCustomGoal(s.metric, period, target, label, pin) }
                },
                onSave = { _, _ -> },
                onDelete = {},
                modifier = body
            )
            is EditorStep.CustomEdit -> {
                val key = customPinKey(s.goal.id)
                val wasPinned = key in livePinned
                CustomTargetStep(
                    metric = s.goal.metric,
                    goal = s.goal,
                    loadHistory = viewModel::customHistory,
                    loadWeighIns = viewModel::weighIns90,
                    pinned = wasPinned,
                    otherPins = livePinned.count { it != key },
                    onCreate = { _, _, _, _ -> },
                    onSave = { target, pin ->
                        commit {
                            if (target != null) viewModel.updateCustomGoalTarget(s.goal.id, target)
                            if (pin != wasPinned) viewModel.toggleCustomPin(s.goal.id)
                        }
                    },
                    onDelete = { commit { viewModel.deleteCustomGoal(s.goal.id) } },
                    modifier = body
                )
            }
        }
    }
}

// ─── Add a goal ─────────────────────────────────────────────────────────────

/** One tile of the goal-kind chooser: its glyph, its name, and a real example of what it tracks. */
private data class GoalKind(val metric: GoalMetric?, val label: String, val example: String, val icon: ImageVector)

/**
 * Add a goal: six kinds as quiet outlined tiles, two across, each with an example of the number it
 * will hold. A lift target goes on to the lift picker; every other kind straight to its target.
 */
@Composable
internal fun ChooseTypeStep(onPickLift: () -> Unit, onPickMetric: (GoalMetric) -> Unit, modifier: Modifier = Modifier) {
    val settings = LocalForgeSettings.current
    val unit = settings.weightUnit
    val kinds = remember(unit, settings.useMiles) {
        val kg = unit == WeightUnit.KG
        listOf(
            GoalKind(null, "Lift target", if (kg) "e.g. Bench 100 kg" else "e.g. Bench 225 lb", OpenIcons.Dumbbell),
            GoalKind(GoalMetric.VOLUME, "Weekly volume", if (kg) "e.g. 27k kg a week" else "e.g. 60k lb a week", OpenIcons.BarChart),
            GoalKind(GoalMetric.SESSIONS, "Workouts", "e.g. 5 a week", OpenIcons.Calendar),
            GoalKind(GoalMetric.BODYWEIGHT, "Body weight", if (kg) "e.g. 80 kg" else "e.g. 178 lb", OpenIcons.User),
            GoalKind(GoalMetric.CARDIO_DISTANCE, "Cardio distance", if (settings.useMiles) "e.g. 10 mi a week" else "e.g. 15 km a week", OpenIcons.Navigation),
            GoalKind(GoalMetric.CARDIO_MINUTES, "Cardio time", "e.g. 150 min a week", OpenIcons.Timer),
        ).filter { it.metric == null || it.metric in customGoalMetrics }
    }
    Column(
        modifier
            .verticalScroll(rememberScrollState())
            .padding(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 40.dp)
    ) {
        Spacer(Modifier.height(20.dp))
        Text("Add a goal", style = GoalsType.pageTitle, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.semantics { heading() })
        Spacer(Modifier.height(8.dp))
        OpenLabel("Pick what to aim for · the number comes next", style = OpenType.monoXS)
        Spacer(Modifier.height(32.dp))
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            kinds.chunked(2).forEach { pair ->
                Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    pair.forEach { kind ->
                        KindTile(
                            kind,
                            onClick = { if (kind.metric == null) onPickLift() else onPickMetric(kind.metric) },
                            modifier = Modifier.weight(1f).fillMaxHeight()
                        )
                    }
                    if (pair.size == 1) Spacer(Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
private fun KindTile(kind: GoalKind, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(4.dp)
    Column(
        modifier
            .bounceClick(role = Role.Button, onClick = onClick)
            .border(1.dp, OpenColors.Track, shape)
            .semantics(mergeDescendants = true) { contentDescription = "${kind.label}, ${kind.example}" }
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Icon(kind.icon, contentDescription = null, tint = MaterialTheme.colorScheme.onBackground, modifier = Modifier.size(22.dp))
        Text(kind.label, style = OpenType.rowTitleL, color = MaterialTheme.colorScheme.onBackground)
        OpenLabel(kind.example, style = OpenType.monoXS)
    }
}

// ─── Pick a lift ────────────────────────────────────────────────────────────

/**
 * For a lift target: a searchable single-select over the whole library, the lifts you train first.
 * [exclude] drops exercises that already have a goal and ones you've hidden in Exercise likes.
 */
@Composable
private fun LiftPickerStep(
    exclude: Set<String>,
    trainedFirst: List<String>,
    onPick: (id: String, name: String) -> Unit,
    modifier: Modifier = Modifier,
) {
    // Saveable like everything else in the editor, so a rotation keeps the search typed so far.
    var query by rememberSaveable { mutableStateOf("") }
    val results = remember(query, exclude, trainedFirst) {
        val rank = trainedFirst.withIndex().associate { (i, id) -> id to i }
        // Stable: within each part the search's own order (name matches first) stands.
        filterLibrary(query, exclude).sortedBy { rank[it.id] ?: Int.MAX_VALUE }
    }
    val ink = MaterialTheme.colorScheme.onBackground
    LazyColumn(modifier, contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 40.dp)) {
        item("head") {
            Column {
                Spacer(Modifier.height(20.dp))
                OpenLabel("Lift target", style = OpenType.monoXS)
                Spacer(Modifier.height(8.dp))
                Text("Pick a lift", style = GoalsType.pageTitle, color = ink, modifier = Modifier.semantics { heading() })
                Spacer(Modifier.height(24.dp))
                ForgeSearchRow(query, { query = it }, "Search exercises or muscles")
                Spacer(Modifier.height(12.dp))
            }
        }
        items(results, key = { it.id }) { def ->
            Column(
                Modifier
                    .fillMaxWidth()
                    .clickableLabeled(def.name) { onPick(def.id, def.name) }
                    .padding(vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(3.dp)
            ) {
                Text(def.name, style = OpenType.rowTitleL, color = ink)
                OpenLabel(def.muscle.displayName, style = OpenType.monoXS)
            }
        }
        if (results.isEmpty()) {
            item("none") { OpenLabel("No lift matches that", style = OpenType.monoXS, modifier = Modifier.padding(vertical = 16.dp)) }
        }
    }
}

// ─── Set the target ─────────────────────────────────────────────────────────

/** Everything the target page draws, in display units. */
internal data class TargetUi(
    val eyebrow: String,
    val title: String,
    val nowLine: String?,
    val targetLabel: String,
    val valueText: String,
    val unitWord: String,
    val scale: TargetScale,
    val value: Double,
    val sideTop: String?,
    val sideBottom: String?,
    val axisLeft: String,
    val axisLeftIsNow: Boolean,
    val paceLabel: String,
    val paceRight: String?,
    val chartCaption: String?,
    val pinned: Boolean,
    val pinCaption: String,
    val primary: String,
    val destructive: String?,
)

/** The pin row's caption: how many of Home's three are in use, and what a fourth pin does. */
internal fun pinCaption(pinned: Boolean, otherPins: Int): String {
    val used = (otherPins + if (pinned) 1 else 0).coerceAtMost(HOME_PIN_SLOTS)
    return if (!pinned && otherPins >= HOME_PIN_SLOTS) "Home has $HOME_PIN_SLOTS of $HOME_PIN_SLOTS · pinning drops the oldest"
    else "Shows in Home's goals ($used of $HOME_PIN_SLOTS used)"
}

/**
 * The page every kind of goal is set on: an eyebrow and the goal's name, where you are now, the
 * target as a big serif value over the red slider with named ends, what the target means at your
 * pace with its bars, Pin to Home, and Set goal.
 */
@Composable
internal fun TargetPage(
    ui: TargetUi,
    onValue: (Double) -> Unit,
    onTogglePin: () -> Unit,
    onPrimary: () -> Unit,
    onDestructive: () -> Unit,
    modifier: Modifier = Modifier,
    periodSwitch: (@Composable () -> Unit)? = null,
    chart: @Composable () -> Unit,
    extra: (@Composable () -> Unit)? = null,
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(
        modifier
            .verticalScroll(rememberScrollState())
            .padding(start = 24.dp, end = 24.dp, top = 4.dp)
            .navigationBarsPadding()
            .padding(bottom = 32.dp)
    ) {
        Spacer(Modifier.height(20.dp))
        OpenLabel(ui.eyebrow, style = OpenType.monoXS)
        Spacer(Modifier.height(8.dp))
        Text(ui.title, style = GoalsType.pageTitle, color = ink, modifier = Modifier.semantics { heading() })
        ui.nowLine?.let {
            Spacer(Modifier.height(8.dp))
            OpenLabel(it, style = OpenType.monoXS, color = ink)
        }
        Spacer(Modifier.height(32.dp))
        FlowRow(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalArrangement = Arrangement.spacedBy(4.dp),
            itemVerticalAlignment = Alignment.CenterVertically
        ) {
            OpenLabel(ui.targetLabel, Modifier.semantics { heading() })
            periodSwitch?.invoke()
        }
        Spacer(Modifier.height(12.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.Bottom) {
            Text(
                buildAnnotatedString {
                    append(ui.valueText)
                    pushStyle(GoalsType.targetUnit.toSpanStyle().copy(color = muted))
                    append(" ${ui.unitWord}")
                    pop()
                },
                style = GoalsType.target,
                color = ink,
                modifier = Modifier.weight(1f).clearAndSetSemantics { }
            )
            Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(2.dp), modifier = Modifier.padding(bottom = 10.dp)) {
                ui.sideTop?.let { OpenLabel(it, style = OpenType.monoXS, color = ink) }
                ui.sideBottom?.let { OpenLabel(it, style = OpenType.monoXS) }
            }
        }
        Spacer(Modifier.height(20.dp))
        OpenValueSlider(
            fraction = ui.scale.fraction(ui.value),
            onFraction = { onValue(ui.scale.at(it)) },
            description = "Target",
            stateText = "${ui.valueText} ${ui.unitWord}",
            nowMark = ui.axisLeftIsNow,
        )
        Spacer(Modifier.height(8.dp))
        Row(Modifier.fillMaxWidth().clearAndSetSemantics { }, horizontalArrangement = Arrangement.SpaceBetween) {
            OpenLabel(ui.axisLeft, style = OpenType.monoXS, color = if (ui.axisLeftIsNow) ink else muted)
            OpenLabel(scaleLabel(ui.scale.mid), style = OpenType.monoXS)
            OpenLabel(scaleLabel(ui.scale.hi), style = OpenType.monoXS)
        }
        Spacer(Modifier.height(40.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
            OpenLabel(ui.paceLabel, Modifier.weight(1f).semantics { heading() })
            ui.paceRight?.let { OpenLabel(it, color = ink) }
        }
        Spacer(Modifier.height(12.dp))
        chart()
        ui.chartCaption?.let {
            Spacer(Modifier.height(8.dp))
            Text(it, style = OpenType.bodyS, color = muted)
        }
        if (extra != null) {
            Spacer(Modifier.height(28.dp))
            extra()
        }
        Spacer(Modifier.height(28.dp))
        Row(
            Modifier
                .fillMaxWidth()
                .toggleableLabeled("Pin to Home", ui.pinned, onTogglePin),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                Text("Pin to Home", style = OpenType.rowTitleL, color = ink)
                OpenLabel(ui.pinCaption, style = OpenType.monoXS)
            }
            OpenSwitchMark(ui.pinned)
        }
        Spacer(Modifier.height(28.dp))
        OpenWideButton(ui.primary, filled = true, onClick = onPrimary, modifier = Modifier.fillMaxWidth())
        ui.destructive?.let {
            Spacer(Modifier.height(16.dp))
            OpenSmallButton(it, filled = false, onClick = onDestructive, icon = OpenIcons.Trash)
        }
    }
}

/** A signed change in the display unit: "+20 lb", "−4.5 lb". */
private fun signed(v: Double, unitWord: String): String =
    "${if (v < 0) "−" else "+"}${trimDecimal(kotlin.math.abs(v))} $unitWord"

/** A lift target: from your best set, plate by plate, with the bars your pace draws to it. */
@Composable
internal fun LiftTargetStep(
    step: EditorStep.LiftWeight,
    history: GoalsViewModel.LiftHistory?,
    pinned: Boolean,
    otherPins: Int,
    onSet: (lb: Double, pin: Boolean) -> Unit,
    onClear: () -> Unit,
    modifier: Modifier = Modifier,
    nowMs: Long = System.currentTimeMillis(),
    today: LocalDate = LocalDate.now(),
) {
    val unit = LocalForgeSettings.current.weightUnit
    val locale = currentLocale()
    val u = unitLabel(unit)
    val topSets = history?.topSets.orEmpty()
    val bestLb = history?.bestSet?.first ?: topSets.maxOfOrNull { it.weightLb } ?: 0.0
    val nowD = toDisplayWeight(bestLb, unit)
    val existing = step.currentTargetLb?.let { toDisplayWeight(it, unit) }
    val scale = remember(nowD, existing, unit) { liftScale(nowD, existing, unit) }
    var picked by rememberSaveable(step.exerciseId, unit) { mutableStateOf<Double?>(null) }
    var pin by rememberSaveable(step.exerciseId) { mutableStateOf(pinned) }
    val value = picked ?: existing ?: liftDefaultTarget(scale)
    val perWeekLb = liftPerWeek(topSets, nowMs)
    val targetLb = picked?.let { fromDisplayWeight(it, unit) } ?: step.currentTargetLb ?: fromDisplayWeight(value, unit)
    val eta = etaWeeks(targetLb - bestLb, perWeekLb)
    val bars = remember(topSets, targetLb, perWeekLb, locale) { paceBars(topSets, nowMs, targetLb, perWeekLb, locale) }
    val best = history?.bestSet
    val ui = TargetUi(
        eyebrow = "Lift target",
        title = step.name,
        nowLine = best?.let { (w, r) ->
            "Best now ${trimDecimal(toDisplayWeight(w, unit))} × $r · est. 1RM ${toDisplayWeight(E1rm.epley(w, r), unit).roundToInt()} $u"
        } ?: if (history != null) "Nothing logged on this lift yet" else null,
        targetLabel = "Target · top set weight",
        valueText = scaleLabel(value),
        unitWord = u,
        scale = scale,
        value = value,
        sideTop = if (nowD > 0) signed(value - nowD, u) else null,
        sideBottom = when {
            nowD > 0 && value <= nowD -> "Already lifted"
            eta != null && eta > 0 -> landingText(today, eta, locale)
            else -> null
        },
        axisLeft = if (nowD > 0) "Now ${scaleLabel(nowD)}" else scaleLabel(scale.lo),
        axisLeftIsNow = nowD > 0,
        paceLabel = "At your pace",
        paceRight = when {
            eta == null -> "No pace yet"
            eta == 0.0 -> "Done"
            else -> etaText(eta)
        },
        chartCaption = if (bars.isEmpty()) "Log this lift and your pace shows here."
        else if (bars.any { it.projected }) "Red is done, grey is your pace." else "Red is your best top set by fortnight.",
        pinned = pin,
        pinCaption = pinCaption(pin, otherPins),
        primary = "Set goal",
        destructive = if (step.currentTargetLb != null) "Clear goal" else null,
    )
    TargetPage(
        ui = ui,
        onValue = { picked = it },
        onTogglePin = { pin = !pin },
        onPrimary = { if (targetLb > 0) onSet(targetLb, pin) },
        onDestructive = onClear,
        modifier = modifier,
        chart = {
            FloorColumns(
                values = bars.map { toDisplayWeight(it.valueLb, unit) },
                lit = bars.map { !it.projected },
                labels = bars.map { it.label },
                fromZero = false,
                description = "Top set by fortnight: done in red, at your pace in grey",
            )
        }
    )
}

/** A metric value in its display unit. */
private fun toDisplay(metric: GoalMetric, v: Double, unit: WeightUnit, useMiles: Boolean): Double = when (metric) {
    GoalMetric.CARDIO_DISTANCE -> toDisplayDistance(v, useMiles)
    GoalMetric.VOLUME, GoalMetric.BODYWEIGHT -> toDisplayWeight(v, unit)
    else -> v
}

private fun fromDisplay(metric: GoalMetric, v: Double, unit: WeightUnit, useMiles: Boolean): Double = when (metric) {
    GoalMetric.CARDIO_DISTANCE -> fromDisplayDistance(v, useMiles)
    GoalMetric.VOLUME, GoalMetric.BODYWEIGHT -> fromDisplayWeight(v, unit)
    else -> v
}

/** An average to one decimal, so it agrees with the "+0.9 on avg" beside the target. */
private fun avgText(metric: GoalMetric, v: Double): String =
    if (metric == GoalMetric.VOLUME) metricValueText(metric, v) else trimDecimal(v)

/** A display value as the big figure: volume in thousands ("60k"), the rest as numbers. */
private fun metricValueText(metric: GoalMetric, v: Double): String = when (metric) {
    GoalMetric.VOLUME -> if (v >= 1000) "${trimDecimal(v / 1000)}k" else v.roundToInt().toString()
    GoalMetric.SESSIONS, GoalMetric.CARDIO_MINUTES -> v.roundToInt().toString()
    else -> trimDecimal(v)
}

private fun metricUnitWord(metric: GoalMetric, unit: WeightUnit, useMiles: Boolean, v: Double): String = when (metric) {
    GoalMetric.SESSIONS -> if (v.roundToInt() == 1) "workout" else "workouts"
    GoalMetric.CARDIO_MINUTES -> "min"
    GoalMetric.CARDIO_DISTANCE -> distanceUnitLabel(useMiles)
    GoalMetric.VOLUME, GoalMetric.BODYWEIGHT -> unitLabel(unit)
}

private fun periodWord(period: GoalPeriod): String = when (period) {
    GoalPeriod.WEEK -> "a week"
    GoalPeriod.MONTH -> "a month"
    GoalPeriod.ALL -> "in all"
}

private fun kindName(metric: GoalMetric): String = when (metric) {
    GoalMetric.VOLUME -> "Weekly volume"
    GoalMetric.BODYWEIGHT -> "Body weight"
    else -> metricDisplayName(metric)
}

/**
 * Any other kind of goal: its windows (week, month or all time) for a running total, or the
 * weigh-ins for body weight, set on the same page as a lift.
 */
@Composable
internal fun CustomTargetStep(
    metric: GoalMetric,
    goal: ExtendedGoalRepository.Progress?,
    loadHistory: suspend (GoalMetric, GoalPeriod) -> List<Double>,
    loadWeighIns: suspend () -> List<Pair<Long, Double>>,
    pinned: Boolean,
    otherPins: Int,
    onCreate: (period: GoalPeriod, target: Double, label: String, pin: Boolean) -> Unit,
    onSave: (target: Double?, pin: Boolean) -> Unit,
    onDelete: () -> Unit,
    modifier: Modifier = Modifier,
    today: LocalDate = LocalDate.now(),
    initialHistory: List<Double>? = null,
    initialWeighIns: List<Pair<Long, Double>>? = null,
) {
    val settings = LocalForgeSettings.current
    val unit = settings.weightUnit
    val miles = settings.useMiles
    val locale = currentLocale()
    var period by rememberSaveable(metric) {
        mutableStateOf(goal?.period ?: if (metric == GoalMetric.BODYWEIGHT) GoalPeriod.ALL else GoalPeriod.WEEK)
    }
    val history = produceState(initialHistory, metric, period) {
        if (initialHistory == null && metric != GoalMetric.BODYWEIGHT) value = loadHistory(metric, period)
    }.value
    val weighIns = produceState(initialWeighIns, metric) {
        if (initialWeighIns == null && metric == GoalMetric.BODYWEIGHT) value = loadWeighIns()
    }.value.orEmpty()
    val recent = if (metric == GoalMetric.BODYWEIGHT) weighIns.map { toDisplayWeight(it.second, unit) }
    else history.orEmpty().map { toDisplay(metric, it, unit, miles) }
    val existing = goal?.targetValue?.let { toDisplay(metric, it, unit, miles) }
    val bodyNow = if (metric == GoalMetric.BODYWEIGHT) (goal?.currentValue?.takeIf { it > 0 }?.let { toDisplayWeight(it, unit) } ?: recent.lastOrNull()) else null
    val scale = remember(metric, period, recent, existing, bodyNow) {
        if (metric == GoalMetric.BODYWEIGHT) bodyweightScale(bodyNow ?: 0.0, existing, unit)
        else cumulativeScale(metric, period, recent, existing, unit)
    }
    var picked by rememberSaveable(metric, period, unit, miles) { mutableStateOf<Double?>(null) }
    var pin by rememberSaveable(metric, goal?.id) { mutableStateOf(pinned) }
    var name by rememberSaveable(metric) { mutableStateOf("") }
    val value = picked ?: existing ?: if (metric == GoalMetric.BODYWEIGHT) {
        scale.snap((bodyNow ?: scale.mid) - if (unit == WeightUnit.LB) 5.0 else 2.5)
    } else cumulativeDefaultTarget(metric, period, recent, scale, unit)
    val unitWord = metricUnitWord(metric, unit, miles, value)
    val valueText = metricValueText(metric, value)
    val focus = LocalFocusManager.current

    val ui = when {
        metric == GoalMetric.BODYWEIGHT -> {
            val u = unitLabel(unit)
            val change = weighInChange30d(weighIns)?.let { toDisplayWeight(it, unit) }
            val perWeek = change?.let { it / 30.0 * 7.0 } ?: 0.0
            val now = bodyNow
            val toward = if (now != null && value < now) -perWeek else perWeek
            val eta = now?.let { etaWeeks(kotlin.math.abs(value - it), toward) }
            TargetUi(
                eyebrow = if (goal == null) "New goal" else "Body weight",
                title = if (goal == null) "Body weight" else customGoalTitle(goal),
                nowLine = now?.let { "Now ${trimDecimal(it)} $u" } ?: "No weigh-in yet",
                targetLabel = "Target · body weight",
                valueText = valueText,
                unitWord = unitWord,
                scale = scale,
                value = value,
                sideTop = now?.let { signed(value - it, u) },
                sideBottom = eta?.takeIf { it > 0 }?.let { landingText(today, it, locale) },
                axisLeft = scaleLabel(scale.lo),
                axisLeftIsNow = false,
                paceLabel = "At your pace",
                paceRight = when {
                    eta == null -> "No pace yet"
                    eta == 0.0 -> "Done"
                    else -> etaText(eta)
                },
                chartCaption = when {
                    weighIns.size < 2 -> "Weigh in a few times and your pace shows here."
                    change != null -> "Your weigh-ins, the last 90 days · ${changeLine(change).lowercase()}."
                    else -> "Your weigh-ins, the last 90 days."
                },
                pinned = pin,
                pinCaption = pinCaption(pin, otherPins),
                primary = if (goal == null) "Set goal" else "Save",
                destructive = if (goal == null) null else "Delete goal",
            )
        }
        else -> {
            val done = recent.dropLast(1).filter { it > 0 }
            val avg = done.takeIf { it.isNotEmpty() }?.average()
            val windowWord = if (period == GoalPeriod.MONTH) "month" else "week"
            TargetUi(
                eyebrow = if (goal == null) "New goal" else kindName(metric),
                title = if (goal == null) kindName(metric) else customGoalTitle(goal),
                nowLine = when {
                    period == GoalPeriod.ALL -> goal?.let { "So far ${customTargetText(metric, it.currentValue, unit, miles)}" }
                    recent.isNotEmpty() -> buildString {
                        append("This $windowWord ${metricValueText(metric, recent.last())}")
                        avg?.let { append(" · avg ${avgText(metric, it)} ${periodWord(period)}") }
                    }
                    else -> null
                },
                targetLabel = "Target · ${unitWord} ${periodWord(period)}",
                valueText = valueText,
                unitWord = unitWord,
                scale = scale,
                value = value,
                sideTop = avg?.let { a -> if (kotlin.math.abs(value - a) < scale.step / 2) "Your average" else "${signed(value - a, "").trim()} on avg" },
                sideBottom = periodWord(period).replaceFirstChar { it.uppercase() },
                axisLeft = scaleLabel(scale.lo),
                axisLeftIsNow = false,
                paceLabel = when (period) {
                    GoalPeriod.WEEK -> "Your last ${recent.size} weeks"
                    GoalPeriod.MONTH -> "Your last ${recent.size} months"
                    GoalPeriod.ALL -> "All time"
                },
                paceRight = avg?.let { "Avg ${avgText(metric, it)}" },
                chartCaption = when {
                    period == GoalPeriod.ALL -> "An all-time total keeps counting from everything you log."
                    recent.all { it <= 0.0 } -> "Nothing logged in these ${windowWord}s yet."
                    else -> "Red ${windowWord}s reached ${valueText} ${unitWord}."
                },
                pinned = pin,
                pinCaption = pinCaption(pin, otherPins),
                primary = if (goal == null) "Set goal" else "Save",
                destructive = if (goal == null) null else "Delete goal",
            )
        }
    }
    val monthLabels = remember(period, recent.size, locale, today) {
        recent.indices.map { i ->
            val back = recent.size - 1 - i
            when {
                back == 0 -> "Now"
                period == GoalPeriod.MONTH -> today.minusMonths(back.toLong()).month.getDisplayName(java.time.format.TextStyle.SHORT, locale)
                i == 0 -> "${back}w"
                else -> ""
            }
        }
    }
    TargetPage(
        ui = ui,
        onValue = { picked = it },
        onTogglePin = { pin = !pin },
        onPrimary = {
            val target = fromDisplay(metric, value, unit, miles)
            if (goal == null) onCreate(period, target, name.trim(), pin)
            // An untouched target saves nothing: re-saving a unit-rounded seed drifts the stored value.
            else onSave(if (picked == null) null else target, pin)
        },
        onDestructive = onDelete,
        modifier = modifier,
        periodSwitch = if (goal == null && metric.isCumulative) {
            {
                OpenRangeSwitch(
                    GoalPeriod.entries, period,
                    label = { when (it) { GoalPeriod.WEEK -> "Week"; GoalPeriod.MONTH -> "Month"; GoalPeriod.ALL -> "All" } },
                    onSelect = { period = it }
                )
            }
        } else null,
        chart = {
            when {
                metric == GoalMetric.BODYWEIGHT -> if (recent.size >= 2) {
                    OpenSparkline(recent.map { it.toFloat() }, Modifier.fillMaxWidth().height(80.dp), description = "Your weigh-ins")
                } else FloorColumns(emptyList(), emptyList(), emptyList(), fromZero = true, description = "No weigh-ins yet")
                period == GoalPeriod.ALL -> Unit
                else -> FloorColumns(
                    values = recent,
                    lit = recent.map { it >= value - 1e-9 },
                    labels = monthLabels,
                    fromZero = true,
                    description = "Your recent ${if (period == GoalPeriod.MONTH) "months" else "weeks"}, the ones that reached the target in red",
                )
            }
        },
        extra = if (goal == null) {
            {
                OpenField(
                    value = name,
                    onValueChange = { name = it },
                    label = "Goal name",
                    placeholder = "Name it (optional)",
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { focus.clearFocus() })
                )
            }
        } else null
    )
}

/**
 * Columns on a baseline: [lit] ones red, the rest the quiet grey, names under them. With
 * [fromZero] false the floor sits a little under the lowest value, so a lift's 185 → 225 climb
 * reads as the climb it is rather than nine near-equal bars. Empty draws the bare baseline.
 */
@Composable
private fun FloorColumns(
    values: List<Double>,
    lit: List<Boolean>,
    labels: List<String>,
    fromZero: Boolean,
    description: String,
    modifier: Modifier = Modifier,
) {
    val accent = MaterialTheme.colorScheme.primary
    val labelColor = MaterialTheme.colorScheme.onSurfaceVariant
    val measurer = rememberTextMeasurer()
    Canvas(modifier.fillMaxWidth().height(130.dp).clearAndSetSemantics { contentDescription = description }) {
        val labelRoom = 18.dp.toPx()
        val bottom = size.height - labelRoom
        drawRect(OpenColors.Rule, topLeft = Offset(0f, bottom), size = Size(size.width, 1.dp.toPx()))
        if (values.isEmpty()) return@Canvas
        val hi = values.max().takeIf { it > 0 } ?: 1.0
        val lo = if (fromZero) 0.0 else (values.min() - (hi - values.min()) * 0.35 - hi * 0.02).coerceAtLeast(0.0)
        val slot = size.width / values.size
        val bw = minOf(slot * 0.6f, 24.dp.toPx())
        values.forEachIndexed { i, v ->
            val x = i * slot + (slot - bw) / 2f
            val h = if (v <= 0.0) 0f else (((v - lo) / (hi - lo)).toFloat() * (bottom - 4.dp.toPx())).coerceAtLeast(3.dp.toPx())
            if (h > 0f) {
                drawRoundRect(
                    if (lit.getOrElse(i) { false }) accent else OpenColors.Previous,
                    topLeft = Offset(x, bottom - h), size = Size(bw, h), cornerRadius = CornerRadius(2.dp.toPx())
                )
            }
            labels.getOrNull(i)?.takeIf { it.isNotEmpty() }?.let { s ->
                val t = measurer.measure(s, OpenType.chartLabel.copy(color = labelColor))
                val cx = (x + bw / 2f - t.size.width / 2f).coerceIn(0f, size.width - t.size.width)
                drawText(t, topLeft = Offset(cx, bottom + 5.dp.toPx()))
            }
        }
    }
}
