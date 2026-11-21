@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.FlowRowScope
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.domain.schedule.WeeklySchedule
import com.forge.app.program.Equipment
import com.forge.app.program.EquipmentPreset
import com.forge.app.program.equipmentGroups
import com.forge.app.program.equipmentPresets
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.onboarding.EXPERIENCE_DETAILS
import com.forge.app.ui.onboarding.GOAL_DETAILS
import com.forge.app.ui.onboarding.OnboardingIcons
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType

/**
 * Program & equipment (Relay design "sp"): the plan in one line, then what shapes it. The page
 * opens on the plan as a serif line, the days a week as seven chips and the session length beside
 * its name, then the four things that shape the plan (schedule, goal, emphasis, equipment) as rows
 * with their live values, the rotation, and the actions that rebuild it. The open section is
 * hoisted to [SettingsScreen] so every back returns to this page before leaving it.
 */
internal enum class ProgramSection(val title: String) {
    Plan("Schedule"),
    Goal("Goal & experience"),
    Emphasis("Emphasis & priorities"),
    Equipment("Equipment")
}

@Composable
internal fun ProgramPage(
    state: SettingsUiState,
    vm: SettingsViewModel,
    section: ProgramSection?,
    onSectionChange: (ProgramSection?) -> Unit,
    onBack: () -> Unit,
    onOpenBuilder: () -> Unit = {},
    /** The active plan's split ("Upper · Lower", see [splitLabel]). */
    split: String = ""
) {
    SettingsScaffold(section?.title ?: "Program & equipment", onBack) {
        when (section) {
            null -> ProgramMenu(state, vm, split, onOpenBuilder) { onSectionChange(it) }
            ProgramSection.Plan -> PlanSection(state, vm)
            ProgramSection.Goal -> GoalExperienceSection(state, vm)
            ProgramSection.Emphasis -> EmphasisSection(state, vm)
            ProgramSection.Equipment -> EquipmentSection(state, vm)
        }
    }
}

// ─── The page ────────────────────────────────────────────────────────────────

@Composable
private fun ProgramMenu(
    state: SettingsUiState,
    vm: SettingsViewModel,
    split: String,
    onOpenBuilder: () -> Unit,
    onOpen: (ProgramSection) -> Unit
) {
    val scheduleMode by vm.scheduleMode.collectAsState()
    var rebuild by remember { mutableStateOf<Rebuild?>(null) }
    ProgramMenuContent(
        state, scheduleMode, split,
        ProgramMenuActions(
            open = onOpen,
            openBuilder = onOpenBuilder,
            setDays = vm::setDaysPerWeek,
            setMinutes = vm::setSessionMinutes,
            setRotation = vm::setRotationCadence,
            rebuild = { rebuild = Rebuild.GENERATE },
            reroll = { rebuild = Rebuild.REROLL },
            deload = { rebuild = Rebuild.DELOAD }
        )
    )
    rebuild?.let { r -> RebuildConfirm(r, state.daysPerWeek, vm) { rebuild = null } }
}

/** What the Program page's top can change, so it draws without its view model (goldens). */
internal class ProgramMenuActions(
    val open: (ProgramSection) -> Unit = {},
    val openBuilder: () -> Unit = {},
    val setDays: (Int) -> Unit = {},
    val setMinutes: (Int?) -> Unit = {},
    val setRotation: (String, Int) -> Unit = { _, _ -> },
    val rebuild: () -> Unit = {},
    val reroll: () -> Unit = {},
    val deload: () -> Unit = {}
)

/** The Program page's own content (Relay design "sp"), laid into [SettingsScaffold]'s column. */
@Composable
internal fun ProgramMenuContent(
    state: SettingsUiState,
    scheduleMode: String,
    split: String,
    actions: ProgramMenuActions
) {
    val vm = actions
    val onOpen = actions.open
    val onOpenBuilder = actions.openBuilder
    // The plan in one line.
    Column {
        OpenLabel("Your plan", style = OpenType.monoXS)
        Spacer(Modifier.height(8.dp))
        Text(
            if (state.freestyleMode) "Freestyle" else planLine(split, state.daysPerWeek),
            style = OpenType.figureRow,
            color = MaterialTheme.colorScheme.onBackground
        )
        Spacer(Modifier.height(8.dp))
        OpenLabel(
            planCaption(state, scheduleMode),
            style = OpenType.monoXS,
            color = MaterialTheme.colorScheme.onBackground
        )
    }

    // Days and length: the two values the plan is cut to, set right here.
    Column {
        SettingsGroupHeader("Days", trailing = "${state.daysPerWeek} a week")
        DayCountChips(state.daysPerWeek) { vm.setDays(it) }
        Spacer(Modifier.height(10.dp))
        Text(
            "Sets your weekly target on Home. The split changes when you rebuild the plan.",
            style = OpenType.bodyS,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        if (!state.freestyleMode) {
            Spacer(Modifier.height(18.dp))
            val lengths = com.forge.app.program.SessionEstimate.SESSION_LENGTH_CHOICES
            SettingsSegmentedRow(
                "Session length",
                lengths.map { com.forge.app.program.SessionEstimate.sessionLengthLabel(it) },
                lengths.indexOf(state.sessionMinutes).takeIf { it >= 0 } ?: lengths.lastIndex
            ) { vm.setMinutes(lengths[it]) }
        }
    }

    SettingsGroup("What shapes it") {
        SettingsNavigationRow(ProgramSection.Plan.title, planSummary(state, scheduleMode)) { onOpen(ProgramSection.Plan) }
        SettingsNavigationRow(ProgramSection.Goal.title, goalSummary(state)) { onOpen(ProgramSection.Goal) }
        SettingsNavigationRow(ProgramSection.Emphasis.title, emphasisSummary(state)) { onOpen(ProgramSection.Emphasis) }
        SettingsNavigationRow(ProgramSection.Equipment.title, equipmentSummary(state)) { onOpen(ProgramSection.Equipment) }
        // The builder is its own screen, not a fifth section.
        SettingsNavigationRow("Plan builder", "Edit each day and exercise by hand", onClick = onOpenBuilder)
    }

    SettingsGroup("Rotation") {
        SettingsSegmentedRow(
            "Automatic re-roll",
            listOf("Never", "4", "8", "12"),
            when {
                state.rotationCadence != "every_n" -> 0
                else -> listOf(4, 8, 12).indexOf(state.rotationEveryN).coerceAtLeast(0) + 1
            },
            supporting = "After this many finished sessions",
            stacked = true
        ) { i ->
            if (i == 0) vm.setRotation("never", state.rotationEveryN)
            else vm.setRotation("every_n", listOf(4, 8, 12)[i - 1])
        }
    }

    // The one rebuild as drawn, then the two lighter ones as plain words: each replaces the plan
    // too, behind the same confirm.
    Column {
        SettingsButton("Rebuild the plan from these", kind = ButtonKind.Outlined, onClick = vm.rebuild)
        Spacer(Modifier.height(4.dp))
        OpenLabel("History and records stay; only the plan changes", style = OpenType.monoXS)
        Spacer(Modifier.height(12.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(24.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
            ProgramTextAction("Re-roll exercises", vm.reroll)
            ProgramTextAction("Start a deload week", vm.deload)
        }
    }
}

/** A quiet action as words (14/600 ink), a 48dp target. */
@Composable
private fun ProgramTextAction(label: String, onClick: () -> Unit) {
    Text(
        label,
        style = com.forge.app.ui.theme.SettingsType.chip,
        color = MaterialTheme.colorScheme.onBackground,
        modifier = Modifier
            .touchSlop(vertical = 15.dp, horizontal = 4.dp)
            .clickableLabeled(label, onClick = onClick)
            .padding(vertical = 15.dp, horizontal = 4.dp)
    )
}

/** "60 min sessions · in sequence": what the serif line leaves out. */
private fun planCaption(state: SettingsUiState, scheduleMode: String): String = when {
    state.freestyleMode -> "No fixed plan · log as you go"
    else -> buildList {
        add(state.sessionMinutes?.let { "$it min sessions" } ?: "Any session length")
        add(if (scheduleMode == WeeklySchedule.MODE_WEEKDAY) "by weekday" else "in sequence")
    }.joinToString(" · ")
}

/**
 * Days a week as seven chips, 1 to 7, lit up to the count (the design's week of lit days): tap a
 * number to set it. Each is a radio for TalkBack, the picked count the one selected.
 */
@Composable
private fun DayCountChips(days: Int, onSelect: (Int) -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    val shape = RoundedCornerShape(4.dp)
    Row(
        Modifier.fillMaxWidth().selectableGroup().semantics { contentDescription = "Days per week" },
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        (1..7).forEach { n ->
            val lit = n <= days
            Box(
                Modifier
                    .weight(1f)
                    .touchSlop(vertical = 2.dp)
                    .selectable(selected = n == days, role = Role.RadioButton) { onSelect(n) }
                    .padding(vertical = 2.dp)
                    .heightIn(min = 44.dp)
                    .clip(shape)
                    .background(if (lit) accent.copy(alpha = 0.15f) else Color.Transparent)
                    .border(1.dp, if (lit) accent else OpenColors.Rule, shape)
                    .semantics { contentDescription = "$n ${if (n == 1) "day" else "days"} a week" },
                contentAlignment = Alignment.Center
            ) {
                Text(
                    "$n",
                    style = OpenType.monoM.copy(fontWeight = if (lit) FontWeight.Bold else FontWeight.Normal),
                    color = if (lit) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

/** The four Settings actions that replace the plan in one tap. */
private enum class Rebuild { GENERATE, REROLL, DELOAD, REGENERATE }

/**
 * Each of these replaces the plan and drops its overlay: the plan builder's hand edits, "make
 * default" swaps and folded coach changes. None can be undone, so §8's confirm names the act and
 * what is lost. The workout guard still runs behind it when a session is open.
 */
@Composable
private fun RebuildConfirm(
    r: Rebuild,
    days: Int,
    vm: SettingsViewModel,
    onConfirmed: () -> Unit = {},
    onDone: () -> Unit
) {
    val lost = "Hand edits and swaps in the current plan don't carry over. Your history stays."
    val (title, body, label) = when (r) {
        Rebuild.GENERATE -> Triple("Replace your plan?", "A new $days-day split with fresh exercise picks. $lost", "Generate plan")
        Rebuild.REROLL -> Triple("Re-roll every exercise?", "Same split, new movement picks. $lost", "Re-roll exercises")
        Rebuild.DELOAD -> Triple(
            "Start a deload week?",
            "The same movements at lighter volume for a week, then the plan is rebuilt at full volume. $lost",
            "Start deload"
        )
        Rebuild.REGENERATE -> Triple("Rebuild for this equipment?", "A new $days-day plan that uses only the gear you picked. $lost", "Rebuild plan")
    }
    SettingsConfirmDialog(
        title = title,
        body = body,
        confirmLabel = label,
        destructive = false,
        onConfirm = {
            onDone()
            onConfirmed()
            when (r) {
                Rebuild.GENERATE, Rebuild.REGENERATE -> vm.generateProgram(days)
                Rebuild.REROLL -> vm.rerollProgram()
                Rebuild.DELOAD -> vm.generateDeloadWeek()
            }
        },
        onDismiss = onDone
    )
}

/** "In sequence" — the Schedule row's live value. */
private fun planSummary(state: SettingsUiState, scheduleMode: String): String = when {
    state.freestyleMode -> "Freestyle · log as you go"
    scheduleMode == WeeklySchedule.MODE_WEEKDAY -> "Following a plan · by weekday"
    else -> "Following a plan · in sequence"
}

/** "Build muscle · Intermediate" — the Goal & experience row's live value. */
private fun goalSummary(state: SettingsUiState): String {
    val goal = GOAL_DETAILS.firstOrNull { it[0] == state.userGoal }?.get(1) ?: "Build muscle"
    return "$goal · ${state.experience.replaceFirstChar { it.uppercase() }}"
}

/** "Balanced · 2 prioritized" — the Emphasis & priorities row's live value. */
private fun emphasisSummary(state: SettingsUiState): String = buildList {
    add(EMPHASIS_OPTIONS.firstOrNull { it.first == state.programEmphasis }?.second ?: "Balanced")
    add("${state.priorityMuscles.size} prioritized")
    add("${state.problemAreas.size} sore spots")
}.joinToString(" · ")

/** "Basic gym · 45 lb plates" — the Equipment row's live value (preset name when one is active). */
private fun equipmentSummary(state: SettingsUiState): String {
    val base = activePreset(state)?.label
        ?: if (state.availableEquipment.isEmpty()) "All equipment"
        else "${state.availableEquipment.size} pieces"
    return "$base · ${plateHardwareLabel(state.plateWeightLb, state.weightUnit)} plates"
}

internal fun activePreset(state: SettingsUiState): EquipmentPreset? = equipmentPresets.firstOrNull {
    state.availableEquipment == it.equipment && state.frozenExerciseIds == it.frozenIds
}

/** Plates and dumbbells are physical kg/lb hardware — stones has no denomination, so a stones user
 *  sees the lb figures they'd actually load (mirrors PlateCalculatorDialog's isMetric fallback);
 *  showing "3 st 3 lb" for a 45 lb plate would read as nonsense. */
private fun plateHardwareLabel(lb: Double, unit: com.forge.app.domain.units.WeightUnit): String =
    com.forge.app.domain.units.formatWeight(
        lb,
        if (unit.isMetric) com.forge.app.domain.units.WeightUnit.KG else com.forge.app.domain.units.WeightUnit.LB
    )

private val EMPHASIS_OPTIONS = listOf(
    "balanced" to "Balanced", "upper" to "Upper body",
    "legs" to "Legs", "arms-shoulders" to "Arms & shoulders"
)

// ─── Schedule ──────────────────────────────────────────────────────────────────

@Composable
private fun PlanSection(state: SettingsUiState, vm: SettingsViewModel) {
    val mode by vm.scheduleMode.collectAsState()
    val schedule by vm.weeklySchedule.collectAsState()

    SettingsGroup("How you train", rowGap = KitPickGap) {
        SettingsRadioRow("Follow a plan", !state.freestyleMode, "A fixed split the coach adapts week by week") {
            vm.setFreestyleMode(false)
        }
        SettingsRadioRow("Go with the flow", state.freestyleMode, "No fixed plan. Log what you did, whenever.") {
            vm.setFreestyleMode(true)
        }
    }

    // A schedule orders a plan's workouts; freestyle has none to order.
    if (!state.freestyleMode) {
        SettingsGroup("Order") {
            SettingsSegmentedRow(
                "Workouts come",
                listOf("In sequence", "By weekday"),
                if (mode == WeeklySchedule.MODE_WEEKDAY) 1 else 0,
                supporting = if (mode == WeeklySchedule.MODE_WEEKDAY) "Each weekday is pinned to a workout"
                             else "The next workout follows the last one",
                stacked = true
            ) { i -> vm.setScheduleMode(if (i == 1) WeeklySchedule.MODE_WEEKDAY else WeeklySchedule.MODE_SEQUENCE) }
        }

        if (mode == WeeklySchedule.MODE_WEEKDAY) {
            SettingsGroup("Week") {
                val days = com.forge.app.program.Program.days
                val options = listOf("" to "Rest") + days.map { it.key to it.defaultName }
                WEEKDAYS.forEachIndexed { wd, name ->
                    val assigned = schedule.getOrElse(wd) { "" }
                    val index = options.indexOfFirst { it.first == assigned }.coerceAtLeast(0)
                    SettingsDropdownRow(
                        title = name,
                        value = options[index].second,
                        options = options.map { it.second },
                        selectedIndex = index,
                        valueColor = if (index == 0) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onBackground
                    ) { vm.setScheduleDay(wd, options[it].first) }
                }
            }
        }
    }
}

private val WEEKDAYS = listOf("Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday")

// ─── Goal & experience ─────────────────────────────────────────────────────────

/** The same words and rep ranges onboarding asked with (one home for the copy). */
@Composable
private fun GoalExperienceSection(state: SettingsUiState, vm: SettingsViewModel) {
    SettingsGroup("Goal", footer = "Same exercises, different loading. Switch anytime.", rowGap = KitPickGap) {
        GOAL_DETAILS.forEach { (key, label, desc, meta) ->
            SettingsRadioRow(label, state.userGoal == key, "$desc ${meta.replace('-', '–')}.", OnboardingIcons.forGoal(key)) { vm.setUserGoal(key) }
        }
    }
    SettingsGroup("Experience", footer = "Sets your volume and which movements you're given.", rowGap = KitPickGap) {
        EXPERIENCE_DETAILS.forEach { (key, label, desc, meta) ->
            SettingsRadioRow(label, state.experience == key, "$meta · $desc") { vm.setExperience(key) }
        }
    }
}

// ─── Emphasis & priorities ─────────────────────────────────────────────────────

@Composable
private fun EmphasisSection(state: SettingsUiState, vm: SettingsViewModel) {
    SettingsGroup("Emphasis", footer = "A volume lean toward one region.", rowGap = KitPickGap) {
        EMPHASIS_OPTIONS.forEach { (value, label) ->
            SettingsRadioRow(label, state.programEmphasis == value) { vm.setProgramEmphasis(value) }
        }
    }
    SettingsGroup("Priority muscles", footer = "These get extra volume.") {
        SettingsChipBlock {
            com.forge.app.program.MuscleGroup.entries.forEach { m ->
                SettingsFilterChip(m.displayName.sentenceCase(), m.code in state.priorityMuscles) { vm.togglePriorityMuscle(m.code) }
            }
        }
    }
    SettingsGroup("Sore spots", footer = "Flag a sore joint and the plan steers around it.") {
        SettingsChipBlock {
            com.forge.app.program.ProblemArea.entries.forEach { a ->
                SettingsFilterChip(a.displayName.sentenceCase(), a.code in state.problemAreas) { vm.toggleProblemArea(a.code) }
            }
        }
    }
}

// ─── Equipment ─────────────────────────────────────────────────────────────────

/**
 * The generator only picks movements you can do with what's on here, and loads against the plate
 * and dumbbell ceilings below. Setups and the gear grid carry onboarding's glyphs, so the gym you
 * described on day one is edited in the shape you described it.
 */
@Composable
private fun EquipmentSection(state: SettingsUiState, vm: SettingsViewModel) {
    // True once equipment is touched this visit, so the regenerate prompt only speaks up after a change.
    var equipmentEdited by remember { mutableStateOf(false) }
    var regenerate by remember { mutableStateOf(false) }
    val active = activePreset(state)
    if (regenerate) {
        RebuildConfirm(Rebuild.REGENERATE, state.daysPerWeek, vm, onConfirmed = { equipmentEdited = false }) {
            regenerate = false
        }
    }

    SettingsGroup(
        "Setup",
        footer = if (state.frozenExerciseIds != null) "A curated setup locks its exercise list. Change any piece to go custom." else null
    ) {
        TileGrid(equipmentPresets, columns = 2) { preset, mod ->
            GearTile(
                icon = OnboardingIcons.forPreset(preset.id),
                label = preset.label,
                meta = presetMeta(preset),
                selected = active == preset,
                role = Role.RadioButton,
                onClick = { vm.selectEquipmentPreset(preset); equipmentEdited = true },
                modifier = mod
            )
        }
    }

    // One group per kind of gear, the tiles straight in it: no panel inside a panel.
    equipmentGroups.forEach { (group, items) ->
        SettingsGroup(group, headerTrailing = "${items.count { it.name in state.availableEquipment }} of ${items.size}") {
            TileGrid(items, columns = 3) { equip, mod ->
                val selected = equip.name in state.availableEquipment
                GearTile(
                    icon = OnboardingIcons.forEquipment(equip),
                    label = equip.display,
                    selected = selected,
                    role = Role.Checkbox,
                    onClick = {
                        vm.toggleEquipment(equip.name)
                        // The last selected piece can't be removed (the tap is refused), so nothing
                        // changed and there is nothing to regenerate for.
                        if (!(selected && state.availableEquipment.size == 1)) equipmentEdited = true
                    },
                    modifier = mod
                )
            }
        }
    }

    // Hardware comes in the user's own unit, as onboarding offers it: a kilo lifter was shown lb
    // plates read back in kilos ("2.3 kg · 4.5 kg · 20.4 kg"), and since onboarding had stored a
    // real 10 kg plate (22.05 lb), no chip matched it and none showed as selected. Stored in lb;
    // selection compares with a tolerance, since a kilo value is never an exact lb number.
    val metric = state.weightUnit.isMetric
    fun toLb(v: Double) = com.forge.app.domain.units.fromDisplayWeight(v, metric)
    fun same(lb: Double?, v: Double) = lb != null && kotlin.math.abs(lb - toLb(v)) < 0.05
    SettingsGroup("Loading") {
        LoadingChips("Weight per plate") {
            (if (metric) listOf(1.25, 2.5, 5.0, 10.0, 15.0, 20.0, 25.0) else listOf(5.0, 10.0, 15.0, 20.0, 25.0, 45.0)).forEach { v ->
                SettingsFilterChip(plateHardwareLabel(toLb(v), state.weightUnit), same(state.plateWeightLb, v)) { vm.setPlateWeightLb(toLb(v)) }
            }
        }
        LoadingChips("Heaviest dumbbell") {
            SettingsFilterChip("No limit", state.maxDbWeightLb == null) { vm.setMaxDbWeightLb(null) }
            (if (metric) listOf(10.0, 15.0, 20.0, 25.0, 30.0, 40.0, 50.0) else listOf(15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 75.0, 100.0)).forEach { v ->
                SettingsFilterChip(plateHardwareLabel(toLb(v), state.weightUnit), same(state.maxDbWeightLb, v)) { vm.setMaxDbWeightLb(toLb(v)) }
            }
        }
    }

    Column {
        SettingsButton(
            "Regenerate for this equipment",
            kind = if (equipmentEdited) ButtonKind.Primary else ButtonKind.Outlined
        ) {
            regenerate = true
        }
        SettingsGroupFooter(
            if (equipmentEdited) "Equipment changed. Regenerate so your plan uses only what you have."
            else "Changed your gear elsewhere? Regenerate so the plan matches it."
        )
    }
}

/** A titled chip set inside the Loading group. */
@Composable
private fun LoadingChips(title: String, chips: @Composable FlowRowScope.() -> Unit) {
    Column {
        Text(title, style = OpenType.rowTitleL, color = MaterialTheme.colorScheme.onBackground)
        Spacer(Modifier.height(12.dp))
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            content = chips
        )
    }
}

/** Meta line for a preset tile: curated flag or plain piece count. */
private fun presetMeta(preset: EquipmentPreset): String = when {
    preset.frozenIds != null -> "Curated · ${preset.equipment.size} pieces"
    preset.equipment.size == Equipment.entries.size -> "All ${preset.equipment.size} pieces"
    preset.equipment.size == 1 -> "1 piece"
    else -> "${preset.equipment.size} pieces"
}

/**
 * A pick tile for a setup or a piece of gear: its glyph and name, the accent wash, edge and a check
 * once it is on. [role] tells TalkBack whether it is one-of (a setup) or any-of (gear).
 */
@Composable
private fun GearTile(
    icon: ImageVector,
    label: String,
    selected: Boolean,
    role: Role,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    meta: String? = null
) {
    val scheme = MaterialTheme.colorScheme
    val shape = RoundedCornerShape(4.dp)
    val select = if (role == Role.Checkbox) {
        Modifier.toggleable(value = selected, role = role, onValueChange = { onClick() })
    } else {
        Modifier.selectable(selected = selected, role = role, onClick = onClick)
    }
    // The chips' look at tile size: the rule-grey edge, the accent wash under an accent edge once on.
    Box(
        modifier
            .clip(shape)
            .background(if (selected) kitWash() else Color.Transparent)
            .border(1.dp, if (selected) scheme.primary else OpenColors.Rule, shape)
            .then(select)
    ) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 14.dp),
            horizontalAlignment = if (meta == null) Alignment.CenterHorizontally else Alignment.Start,
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Icon(icon, contentDescription = null, tint = if (selected) scheme.onBackground else scheme.onSurfaceVariant, modifier = Modifier.size(24.dp))
            Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                // minLines keeps a row of tiles one height; no maxLines, so long names wrap at 200%.
                Text(
                    label,
                    style = if (meta == null) OpenType.bodyS else OpenType.rowTitleL,
                    color = if (selected) scheme.onBackground else scheme.onSurfaceVariant,
                    textAlign = if (meta == null) TextAlign.Center else TextAlign.Start,
                    minLines = if (meta == null) 2 else 1
                )
                if (meta != null) SettingsReading(meta)
            }
        }
        if (selected) {
            Icon(
                OpenIcons.Check,
                contentDescription = null,
                tint = scheme.primary,
                modifier = Modifier.align(Alignment.TopEnd).padding(8.dp).size(16.dp)
            )
        }
    }
}

/** Equal-width tiles, [columns] per row, a short last row padded so its tiles keep their width. */
@Composable
private fun <T> TileGrid(items: List<T>, columns: Int, tile: @Composable (T, Modifier) -> Unit) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        items.chunked(columns).forEach { row ->
            Row(Modifier.fillMaxWidth().height(IntrinsicSize.Max), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                row.forEach { tile(it, Modifier.weight(1f).fillMaxHeight()) }
                repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}
