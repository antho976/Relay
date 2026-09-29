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
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.TrendingDown
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.EditNote
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.Tune
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.domain.schedule.WeeklySchedule
import com.forge.app.program.Equipment
import com.forge.app.program.EquipmentPreset
import com.forge.app.program.equipmentGroups
import com.forge.app.program.equipmentPresets
import com.forge.app.ui.onboarding.EXPERIENCE_DETAILS
import com.forge.app.ui.onboarding.GOAL_DETAILS
import com.forge.app.ui.onboarding.OnboardingIcons

/**
 * Program & equipment: what the plan is built from. A menu of four focused sections (Plan, Goal &
 * experience, Emphasis & priorities, Equipment), each row carrying its live value, then rotation
 * and the three actions that rebuild the plan. The open section is hoisted to [SettingsScreen] so
 * every back returns to this menu before leaving it.
 */
internal enum class ProgramSection(val title: String) {
    Plan("Plan"),
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
    onOpenBuilder: () -> Unit = {}
) {
    SettingsScaffold(section?.title ?: "Program & equipment", onBack) {
        when (section) {
            null -> ProgramMenu(state, vm, onOpenBuilder) { onSectionChange(it) }
            ProgramSection.Plan -> PlanSection(state, vm)
            ProgramSection.Goal -> GoalExperienceSection(state, vm)
            ProgramSection.Emphasis -> EmphasisSection(state, vm)
            ProgramSection.Equipment -> EquipmentSection(state, vm)
        }
    }
}

// ─── Menu root ───────────────────────────────────────────────────────────────

@Composable
private fun ProgramMenu(
    state: SettingsUiState,
    vm: SettingsViewModel,
    onOpenBuilder: () -> Unit,
    onOpen: (ProgramSection) -> Unit
) {
    val scheduleMode by vm.scheduleMode.collectAsState()
    SettingsGroup("Your plan") {
        SettingsNavigationRow("Plan", planSummary(state, scheduleMode), SettingsIcons.Program) { onOpen(ProgramSection.Plan) }
        SettingsNavigationRow("Goal & experience", goalSummary(state), OnboardingIcons.forGoal(state.userGoal)) { onOpen(ProgramSection.Goal) }
        SettingsNavigationRow("Emphasis & priorities", emphasisSummary(state), Icons.Rounded.Tune) { onOpen(ProgramSection.Emphasis) }
        SettingsNavigationRow("Equipment", equipmentSummary(state), OnboardingIcons.forPreset(activePreset(state)?.id ?: "everything")) {
            onOpen(ProgramSection.Equipment)
        }
        // The builder is its own screen, not a fifth section.
        SettingsNavigationRow("Plan builder", "Edit each day and exercise by hand", Icons.Rounded.EditNote, onClick = onOpenBuilder)
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
            if (i == 0) vm.setRotationCadence("never", state.rotationEveryN)
            else vm.setRotationCadence("every_n", listOf(4, 8, 12)[i - 1])
        }
    }

    SettingsGroup("Rebuild", footer = "A new plan replaces the current one, built from the settings on this page.") {
        SettingsActionRow("Generate a new ${state.daysPerWeek}-day plan", "A fresh split and exercise picks", Icons.Rounded.AutoAwesome) {
            vm.generateProgram(state.daysPerWeek)
        }
        SettingsActionRow("Re-roll exercises now", "Same split, new movement picks", Icons.Rounded.Shuffle) { vm.rerollProgram() }
        SettingsActionRow("Deload week", "A lighter week to recover", Icons.AutoMirrored.Rounded.TrendingDown) { vm.generateDeloadWeek() }
    }
}

/** "4 days a week · in sequence" — the Plan row's live value. */
private fun planSummary(state: SettingsUiState, scheduleMode: String): String = when {
    state.freestyleMode -> "Freestyle · log as you go"
    scheduleMode == WeeklySchedule.MODE_WEEKDAY -> "${state.daysPerWeek} days a week · by weekday"
    else -> "${state.daysPerWeek} days a week · in sequence"
}

/** "Build muscle · Intermediate" — the Goal & experience row's live value. */
private fun goalSummary(state: SettingsUiState): String {
    val goal = GOAL_DETAILS.firstOrNull { it[0] == state.userGoal }?.get(1) ?: "Build muscle"
    return "$goal · ${state.experience.replaceFirstChar { it.uppercase() }}"
}

/** "Balanced · 2 prioritized" — the Emphasis & priorities row's live value. */
private fun emphasisSummary(state: SettingsUiState): String = buildList {
    add(EMPHASIS_OPTIONS.firstOrNull { it.first == state.programEmphasis }?.second ?: "Balanced")
    if (state.priorityMuscles.isNotEmpty()) add("${state.priorityMuscles.size} prioritized")
    if (state.problemAreas.isNotEmpty()) add("${state.problemAreas.size} flagged")
}.joinToString(" · ")

/** "Basic gym · 45 lb plates" — the Equipment row's live value (preset name when one is active). */
private fun equipmentSummary(state: SettingsUiState): String {
    val base = activePreset(state)?.label
        ?: if (state.availableEquipment.isEmpty()) "All equipment"
        else "${state.availableEquipment.size} pieces"
    return "$base · ${plateHardwareLabel(state.plateWeightLb, state.weightUnit)} plates"
}

private fun activePreset(state: SettingsUiState): EquipmentPreset? = equipmentPresets.firstOrNull {
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

// ─── Plan ──────────────────────────────────────────────────────────────────────

@Composable
private fun PlanSection(state: SettingsUiState, vm: SettingsViewModel) {
    val mode by vm.scheduleMode.collectAsState()
    val schedule by vm.weeklySchedule.collectAsState()

    SettingsGroup("How you train") {
        SettingsRadioRow("Follow a plan", !state.freestyleMode, "A fixed split the coach adapts week by week") {
            vm.setFreestyleMode(false)
        }
        SettingsRadioRow("Go with the flow", state.freestyleMode, "No fixed plan. Log what you did, whenever.") {
            vm.setFreestyleMode(true)
        }
    }

    SettingsGroup("Days per week", footer = "Sets your weekly target on Home. Your split changes the next time you generate.") {
        SettingsGroupBlock(padding = PaddingValues(12.dp)) {
            SettingsSegmented(
                options = DAY_COUNT_OPTIONS,
                selectedIndex = state.daysPerWeek - 1,
                onSelect = { vm.setDaysPerWeek(it + 1) },
                contentDescription = "Days per week"
            )
        }
    }

    // A schedule orders a plan's workouts; freestyle has none to order.
    if (!state.freestyleMode) {
        val lengths = com.forge.app.program.SessionEstimate.SESSION_LENGTH_CHOICES
        SettingsGroup(
            "Minutes per session",
            footer = "Each workout is trimmed to fit: accessories go first, your main lifts stay. " +
                "Used the next time you generate or re-roll."
        ) {
            SettingsGroupBlock(padding = PaddingValues(12.dp)) {
                SettingsSegmented(
                    options = lengths.map { com.forge.app.program.SessionEstimate.sessionLengthLabel(it) },
                    selectedIndex = lengths.indexOf(state.sessionMinutes).takeIf { it >= 0 } ?: lengths.lastIndex,
                    onSelect = { vm.setSessionMinutes(lengths[it]) },
                    contentDescription = "Minutes per session"
                )
            }
        }

        SettingsGroup("Schedule") {
            SettingsSegmentedRow(
                "Order",
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
                        valueColor = if (index == 0) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface
                    ) { vm.setScheduleDay(wd, options[it].first) }
                }
            }
        }
    }
}

/** "1".."7", built once: the segmented control keys its label measurements on this list. */
private val DAY_COUNT_OPTIONS = (1..7).map { "$it" }

private val WEEKDAYS = listOf("Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday")

// ─── Goal & experience ─────────────────────────────────────────────────────────

/** The same words and rep ranges onboarding asked with (one home for the copy). */
@Composable
private fun GoalExperienceSection(state: SettingsUiState, vm: SettingsViewModel) {
    SettingsGroup("Goal", footer = "Same exercises, different loading. Switch anytime.") {
        GOAL_DETAILS.forEach { (key, label, desc, meta) ->
            SettingsRadioRow(label, state.userGoal == key, "$desc ${meta.replace('-', '–')}.", OnboardingIcons.forGoal(key)) { vm.setUserGoal(key) }
        }
    }
    SettingsGroup("Experience", footer = "Sets your volume and which movements you're given.") {
        EXPERIENCE_DETAILS.forEach { (key, label, desc, meta) ->
            SettingsRadioRow(label, state.experience == key, "$meta · $desc") { vm.setExperience(key) }
        }
    }
}

// ─── Emphasis & priorities ─────────────────────────────────────────────────────

@Composable
private fun EmphasisSection(state: SettingsUiState, vm: SettingsViewModel) {
    SettingsGroup("Emphasis", footer = "A volume lean toward one region.") {
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
    SettingsGroup("Problem areas", footer = "Flag a sore joint and the plan steers around it.") {
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
    val active = activePreset(state)

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

    SettingsGroup("Loading") {
        LoadingChips("Weight per plate") {
            listOf(5.0, 10.0, 15.0, 20.0, 25.0, 45.0).forEach { w ->
                SettingsFilterChip(plateHardwareLabel(w, state.weightUnit), state.plateWeightLb == w) { vm.setPlateWeightLb(w) }
            }
        }
        LoadingChips("Heaviest dumbbell") {
            SettingsFilterChip("No limit", state.maxDbWeightLb == null) { vm.setMaxDbWeightLb(null) }
            listOf(15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 75.0, 100.0).forEach { w ->
                SettingsFilterChip(plateHardwareLabel(w, state.weightUnit), state.maxDbWeightLb == w) { vm.setMaxDbWeightLb(w) }
            }
        }
    }

    Column {
        SettingsButton(
            "Regenerate for this equipment",
            Modifier.fillMaxWidth(),
            kind = if (equipmentEdited) ButtonKind.Primary else ButtonKind.Tonal,
            icon = Icons.Rounded.AutoAwesome
        ) {
            vm.generateProgram(state.daysPerWeek); equipmentEdited = false
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
    SettingsGroupBlock(padding = PaddingValues(start = KitRowPad, end = KitRowPad, top = 14.dp, bottom = 8.dp)) {
        Text(title, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface)
        Spacer(Modifier.height(6.dp))
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
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
    val shape = RoundedCornerShape(12.dp)
    val select = if (role == Role.Checkbox) {
        Modifier.toggleable(value = selected, role = role, onValueChange = { onClick() })
    } else {
        Modifier.selectable(selected = selected, role = role, onClick = onClick)
    }
    Box(
        modifier
            .clip(shape)
            .background(if (selected) scheme.primaryContainer else kitRowColor())
            .then(if (selected) Modifier.border(1.dp, scheme.primary, shape) else Modifier)
            .then(select)
    ) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 14.dp),
            horizontalAlignment = if (meta == null) Alignment.CenterHorizontally else Alignment.Start,
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Icon(icon, contentDescription = null, tint = if (selected) scheme.onSurface else scheme.onSurfaceVariant, modifier = Modifier.size(26.dp))
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                // minLines keeps a row of tiles one height; no maxLines, so long names wrap at 200%.
                Text(
                    label,
                    style = if (meta == null) MaterialTheme.typography.bodySmall else MaterialTheme.typography.titleSmall,
                    color = if (selected) scheme.onSurface else scheme.onSurfaceVariant,
                    textAlign = if (meta == null) TextAlign.Center else TextAlign.Start,
                    minLines = if (meta == null) 2 else 1
                )
                if (meta != null) Text(meta, style = MaterialTheme.typography.bodySmall, color = scheme.onSurfaceVariant)
            }
        }
        if (selected) {
            Box(
                Modifier
                    .align(Alignment.TopEnd)
                    .padding(8.dp)
                    .size(18.dp)
                    .clip(CircleShape)
                    .background(scheme.primary),
                contentAlignment = Alignment.Center
            ) {
                Icon(Icons.Rounded.Check, contentDescription = null, tint = scheme.onPrimary, modifier = Modifier.size(12.dp))
            }
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
