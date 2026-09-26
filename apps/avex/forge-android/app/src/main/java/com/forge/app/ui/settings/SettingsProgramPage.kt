@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.FlowRowScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import com.forge.app.ui.common.window.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
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
import androidx.compose.ui.unit.dp
import com.forge.app.domain.schedule.WeeklySchedule
import com.forge.app.program.Equipment
import com.forge.app.program.EquipmentPreset
import com.forge.app.program.equipmentGroups
import com.forge.app.program.equipmentPresets
import com.forge.app.ui.common.ForgeChoiceChip
import com.forge.app.ui.common.ForgeDayChip
import com.forge.app.ui.common.ForgeIconTile
import com.forge.app.ui.common.ForgeOptionCard
import com.forge.app.ui.common.ForgePresetTile
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.onboarding.EXPERIENCE_DETAILS
import com.forge.app.ui.onboarding.GOAL_DETAILS
import com.forge.app.ui.onboarding.OnboardingIcons

/**
 * The "Program & equipment" page — the workspace for building and editing a plan. Four focused
 * sub-pages (Plan · Goal & experience · Emphasis & priorities · Equipment) rendered as menu rows
 * carrying their live values, plus a nav link to the full plan builder; the make/refresh-a-plan
 * actions group at the END of the menu root. Nested navigation is local state, hoisted to
 * [SettingsScreen] so the top-bar back and system back both return here before exiting to Settings.
 *
 * Redesigned 2026-09-25: every choice on these pages is drawn from the shared selectable family
 * (`ui/common/Selectables.kt`) — the same option cards, day chips and gear tiles onboarding asks
 * these exact questions with — instead of Settings' old 10sp UPPERCASE chip walls. A setting and
 * the onboarding question that first set it now look like one control.
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
    modifier: Modifier = Modifier,
    onOpenBuilder: () -> Unit = {}
) {
    when (section) {
        null -> ProgramMenu(state, vm, modifier, onOpenBuilder) { onSectionChange(it) }
        ProgramSection.Plan -> ProgramSectionScaffold(modifier) { PlanSection(state, vm) }
        ProgramSection.Goal -> ProgramSectionScaffold(modifier) { GoalExperienceSection(state, vm) }
        ProgramSection.Emphasis -> ProgramSectionScaffold(modifier) { EmphasisSection(state, vm) }
        ProgramSection.Equipment -> ProgramSectionScaffold(modifier) { EquipmentSection(state, vm) }
    }
}

// ─── Menu root ───────────────────────────────────────────────────────────────

@Composable
private fun ProgramMenu(
    state: SettingsUiState,
    vm: SettingsViewModel,
    modifier: Modifier,
    onOpenBuilder: () -> Unit,
    onOpen: (ProgramSection) -> Unit
) {
    val scheduleMode by vm.scheduleMode.collectAsState()
    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        SettingsPageTitle("Program & equipment", "What your plan is built from.")
        // Drill-ins as menu rows showing their live values (§3) with a leading glyph each, the same
        // language as the main settings list, so they read as sub-pages.
        SettingsSectionHeader("Your plan")
        SettingsNavRow("Plan", planSummary(state, scheduleMode), SettingsIcons.Program) { onOpen(ProgramSection.Plan) }
        SettingsNavRow("Goal & experience", goalSummary(state), OnboardingIcons.forGoal(state.userGoal)) { onOpen(ProgramSection.Goal) }
        SettingsNavRow("Emphasis & priorities", emphasisSummary(state), OnboardingIcons.Muscle) { onOpen(ProgramSection.Emphasis) }
        SettingsNavRow("Equipment", equipmentSummary(state), OnboardingIcons.forPreset(activePreset(state)?.id ?: "everything")) {
            onOpen(ProgramSection.Equipment)
        }
        // The builder is a routed screen, not a fifth sub-page — nav renders as an `action →` link (§8 ③).
        SettingsActionLink("Open the plan builder →") { onOpenBuilder() }

        SettingsSectionHeader("Rotation")
        SettingsSegmentRow(
            "Re-roll exercises",
            listOf("Never", "4", "8", "12"),
            when {
                state.rotationCadence != "every_n" -> 0
                else -> listOf(4, 8, 12).indexOf(state.rotationEveryN).coerceAtLeast(0) + 1
            },
            explainer = "After this many finished sessions."
        ) { i ->
            if (i == 0) vm.setRotationCadence("never", state.rotationEveryN)
            else vm.setRotationCadence("every_n", listOf(4, 8, 12)[i - 1])
        }

        // The plan actions (Generate + its re-roll and deload sidekicks) group together at the very
        // END of the page, never mid-scroll (§8).
        SettingsSectionHeader("New plan")
        SettingsCaption("Builds a fresh plan from these settings, replacing the current one.")
        SettingsActionRow {
            SettingsPrimaryAction("Generate ${state.daysPerWeek}-day plan") { vm.generateProgram(state.daysPerWeek) }
            SettingsOutlineAction("Re-roll exercises") { vm.rerollProgram() }
            SettingsOutlineAction("Deload week") { vm.generateDeloadWeek() }
        }
        Spacer(Modifier.height(32.dp))
    }
}

/** "4 days/week · in sequence" — the Plan row's live value. */
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
    SettingsPageTitle("Plan", planTitleLine(state, mode, schedule))

    SettingsSectionHeader("Training mode")
    CardStack {
        ForgeOptionCard(
            label = "Follow a plan",
            description = "A fixed split the coach adapts week by week.",
            selected = !state.freestyleMode,
            onClick = { vm.setFreestyleMode(false) }
        )
        ForgeOptionCard(
            label = "Go with the flow",
            description = "No fixed plan. Log what you did, whenever.",
            selected = state.freestyleMode,
            onClick = { vm.setFreestyleMode(true) }
        )
    }

    SettingsSectionHeader("Days per week")
    SettingsCaption("Your split follows the count, and it sets your weekly target on Home.")
    Row(
        Modifier.fillMaxWidth().padding(horizontal = SETTINGS_GUTTER),
        horizontalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        // Equal cells, each chip centred in its own: seven 48dp targets overran a 360dp phone when
        // they were simply spaced between the gutters.
        (1..7).forEach { n ->
            Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                ForgeDayChip(n, state.daysPerWeek == n, { vm.setDaysPerWeek(n) })
            }
        }
    }

    // A schedule orders a plan's workouts; freestyle has none to order, so the block steps aside.
    if (!state.freestyleMode) {
        SettingsSectionHeader("Schedule")
        SettingsSegmentRow(
            "Order",
            listOf("In sequence", "By weekday"),
            if (mode == WeeklySchedule.MODE_WEEKDAY) 1 else 0,
            explainer = if (mode == WeeklySchedule.MODE_WEEKDAY) "Each weekday is pinned to a workout."
                        else "The next workout follows the last one.",
            fill = true
        ) { i -> vm.setScheduleMode(if (i == 1) WeeklySchedule.MODE_WEEKDAY else WeeklySchedule.MODE_SEQUENCE) }

        if (mode == WeeklySchedule.MODE_WEEKDAY) {
            SettingsSectionHeader("Weekly plan")
            val days = com.forge.app.program.Program.days
            WEEKDAYS.forEachIndexed { wd, name ->
                val assigned = schedule.getOrElse(wd) { "" }
                WeekdayRow(
                    weekday = name,
                    assignedName = days.firstOrNull { it.key == assigned }?.defaultName,
                    options = days.map { it.key to it.defaultName },
                    selectedKey = assigned,
                    onPick = { vm.setScheduleDay(wd, it) }
                )
            }
        }
    }
}

private val WEEKDAYS = listOf("Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday")

/** The Plan page's one line: which weekdays train (a reading), else how the week runs. */
private fun planTitleLine(state: SettingsUiState, mode: String, schedule: List<String>): String = when {
    state.freestyleMode -> "No fixed plan. Home logs whatever you train."
    mode == WeeklySchedule.MODE_WEEKDAY -> {
        val on = WEEKDAYS.indices.filter { schedule.getOrElse(it) { "" }.isNotBlank() }
        if (on.isEmpty()) "No weekday has a workout yet."
        else "Trains " + on.joinToString(" · ") { WEEKDAYS[it].take(3) }
    }
    else -> "${state.daysPerWeek} workouts a week, run in order."
}

/**
 * One weekday of the fixed plan: the day on the left, its workout (or Rest) on the right, the whole
 * row opening a menu of the plan's workouts. It replaced a Rest-plus-every-workout chip flow per day,
 * which stacked seven chip walls into the tallest block in Settings.
 */
@Composable
private fun WeekdayRow(
    weekday: String,
    assignedName: String?,
    options: List<Pair<String, String>>,
    selectedKey: String,
    onPick: (String) -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    var open by remember { mutableStateOf(false) }
    Box {
        Row(
            Modifier
                .fillMaxWidth()
                .clickableLabeled("$weekday, ${assignedName ?: "rest"}. Change") { open = true }
                .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(weekday, style = MaterialTheme.typography.bodyMedium, color = onBg, modifier = Modifier.weight(1f))
            Text(
                assignedName ?: "Rest",
                style = MaterialTheme.typography.bodyMedium,
                color = if (assignedName != null) onBg else muted
            )
            Text("▾", style = MaterialTheme.typography.labelMedium, color = muted)
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            (listOf("" to "Rest") + options).forEach { (key, label) ->
                DropdownMenuItem(
                    text = {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            Box(Modifier.width(8.dp)) { if (key == selectedKey) StatusDot(active = true, size = 7.dp) }
                            Text(label, style = MaterialTheme.typography.bodyMedium, color = if (key == selectedKey) onBg else muted)
                        }
                    },
                    onClick = { onPick(key); open = false }
                )
            }
        }
    }
}

// ─── Goal & experience ─────────────────────────────────────────────────────────

/** The same cards, words and rep-range metas onboarding asked with (one home for the copy). */
@Composable
private fun GoalExperienceSection(state: SettingsUiState, vm: SettingsViewModel) {
    SettingsPageTitle("Goal & experience", "Same exercises, different loading. Switch anytime.")

    SettingsSectionHeader("Goal")
    CardStack {
        GOAL_DETAILS.forEach { (key, label, desc, meta) ->
            ForgeOptionCard(
                label = label, description = desc, meta = meta,
                icon = OnboardingIcons.forGoal(key),
                selected = state.userGoal == key,
                onClick = { vm.setUserGoal(key) }
            )
        }
    }

    SettingsSectionHeader("Experience")
    SettingsCaption("Sets your volume and which movements you're given.")
    CardStack {
        EXPERIENCE_DETAILS.forEach { (key, label, desc, meta) ->
            ForgeOptionCard(
                label = label, description = desc, meta = meta,
                selected = state.experience == key,
                onClick = { vm.setExperience(key) }
            )
        }
    }
}

// ─── Emphasis & priorities ─────────────────────────────────────────────────────

@Composable
private fun EmphasisSection(state: SettingsUiState, vm: SettingsViewModel) {
    SettingsPageTitle("Emphasis & priorities", emphasisSummary(state))

    SettingsSectionHeader("Emphasis")
    SettingsCaption("A volume lean toward one region.")
    ChipFlow {
        EMPHASIS_OPTIONS.forEach { (value, label) ->
            ForgeChoiceChip(label, state.programEmphasis == value, { vm.setProgramEmphasis(value) })
        }
    }

    SettingsSectionHeader("Priority muscles")
    SettingsCaption("These get extra volume.")
    ChipFlow {
        com.forge.app.program.MuscleGroup.entries.forEach { m ->
            ForgeChoiceChip(m.displayName, m.code in state.priorityMuscles, { vm.togglePriorityMuscle(m.code) })
        }
    }

    SettingsSectionHeader("Problem areas")
    SettingsCaption("Flag a sore joint and the plan steers around it.")
    ChipFlow {
        com.forge.app.program.ProblemArea.entries.forEach { a ->
            ForgeChoiceChip(a.displayName, a.code in state.problemAreas, { vm.toggleProblemArea(a.code) })
        }
    }
}

// ─── Equipment ─────────────────────────────────────────────────────────────────

/**
 * Equipment stays inside Program (the page is "Program & equipment"): the generator only picks
 * movements you can do with what's selected, plus the plate/dumbbell ceilings it loads against.
 * Setup presets and the gear grid are onboarding's own tiles, so the gym you described on day one
 * is edited here in the shape you described it. The one do-it-now action (Regenerate) sits at the
 * END of the page (§8).
 */
@Composable
private fun EquipmentSection(state: SettingsUiState, vm: SettingsViewModel) {
    // Becomes true once equipment is touched this visit, so the regenerate prompt only nags after a change.
    var equipmentEdited by remember { mutableStateOf(false) }
    val active = activePreset(state)

    SettingsPageTitle("Equipment", "Plans only pick movements you can do with what's on.")

    SettingsSectionHeader("Setup")
    if (state.frozenExerciseIds != null) {
        SettingsCaption("A curated setup locks its exercise list. Change any piece to go custom.")
    }
    TileGrid(equipmentPresets, columns = 2) { preset, mod ->
        ForgePresetTile(
            icon = OnboardingIcons.forPreset(preset.id),
            label = preset.label,
            meta = presetMeta(preset),
            selected = active == preset,
            onClick = { vm.selectEquipmentPreset(preset); equipmentEdited = true },
            modifier = mod
        )
    }

    SettingsSectionHeader("Gear")
    equipmentGroups.forEach { (group, items) ->
        SettingsGroupLabel(group, "${items.count { it.name in state.availableEquipment }} on")
        TileGrid(items, columns = 3) { equip, mod ->
            val selected = equip.name in state.availableEquipment
            ForgeIconTile(
                icon = OnboardingIcons.forEquipment(equip),
                label = equip.display,
                selected = selected,
                onClick = {
                    val current = state.availableEquipment.toMutableSet()
                    if (selected) current.remove(equip.name) else current.add(equip.name)
                    vm.setAvailableEquipment(current)
                    equipmentEdited = true
                },
                modifier = mod
            )
        }
    }

    SettingsSectionHeader("Loading")
    SettingsGroupLabel("Weight per plate")
    ChipFlow {
        listOf(5.0, 10.0, 15.0, 20.0, 25.0, 45.0).forEach { w ->
            ForgeChoiceChip(plateHardwareLabel(w, state.weightUnit), state.plateWeightLb == w, { vm.setPlateWeightLb(w) })
        }
    }
    SettingsGroupLabel("Heaviest dumbbell")
    ChipFlow {
        ForgeChoiceChip("No limit", state.maxDbWeightLb == null, { vm.setMaxDbWeightLb(null) })
        listOf(15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 75.0, 100.0).forEach { w ->
            ForgeChoiceChip(plateHardwareLabel(w, state.weightUnit), state.maxDbWeightLb == w, { vm.setMaxDbWeightLb(w) })
        }
    }

    Spacer(Modifier.height(28.dp))
    SettingsCaption(
        if (equipmentEdited) "Equipment changed. Regenerate so your plan uses only what you have."
        else "Changed your gear elsewhere? Regenerate so the plan matches it."
    )
    SettingsActionRow {
        SettingsPrimaryAction("Regenerate for this equipment") {
            vm.generateProgram(state.daysPerWeek); equipmentEdited = false
        }
    }
}

/** Mono meta line for a preset tile: curated flag or plain piece count. */
private fun presetMeta(preset: EquipmentPreset): String = when {
    preset.frozenIds != null -> "Curated · ${preset.equipment.size} pieces"
    preset.equipment.size == Equipment.entries.size -> "All ${preset.equipment.size} pieces"
    preset.equipment.size == 1 -> "1 piece"
    else -> "${preset.equipment.size} pieces"
}

// ─── Shared layout helpers ───────────────────────────────────────────────────

/** A plain scrolling container for a Program sub-section. Back is the top-bar `←` alone (one per
 *  page, §4.6); each section opens with its own serif page title. */
@Composable
private fun ProgramSectionScaffold(modifier: Modifier, content: @Composable () -> Unit) {
    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        content()
        Spacer(Modifier.height(32.dp))
    }
}

/** Option cards stacked inside the gutter, 8dp apart. */
@Composable
private fun CardStack(content: @Composable () -> Unit) {
    Column(
        Modifier.fillMaxWidth().padding(horizontal = SETTINGS_GUTTER),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) { content() }
}

/** Equal-width tiles, [columns] per row, a short last row padded so its tiles keep their width. */
@Composable
private fun <T> TileGrid(items: List<T>, columns: Int, tile: @Composable (T, Modifier) -> Unit) {
    Column(
        Modifier.fillMaxWidth().padding(horizontal = SETTINGS_GUTTER),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        items.chunked(columns).forEach { row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                row.forEach { tile(it, Modifier.weight(1f)) }
                repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

/** A wrapping row of choice chips inside the gutter. The chips carry their own 48dp targets, so
 *  the vertical gap stays 0 — the interaction boxes already space the lines. */
@Composable
internal fun ChipFlow(content: @Composable FlowRowScope.() -> Unit) {
    FlowRow(
        modifier = Modifier.padding(horizontal = SETTINGS_GUTTER),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(0.dp),
        content = content
    )
}
