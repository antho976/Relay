@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.domain.schedule.WeeklySchedule

/**
 * The "Program & equipment" page — the workspace for building and editing a plan. A small menu of
 * focused sub-pages (Plan · Focus · Equipment), with the make/refresh-a-plan actions living at the
 * menu root, always one tap away. Coach configuration used to live here too but moved to its own
 * top-level page — this page is only the things that shape a plan. Nested navigation is local state
 * (mirrors the Exercise-likes page), hoisted to [SettingsScreen] so the top-bar back and system back
 * both return here (the menu) before exiting to Settings.
 */
internal enum class ProgramSection(val title: String) {
    Plan("Plan"),
    Goal("Goal & experience"),
    Emphasis("Emphasis & priorities"),
    Equipment("Equipment"),
    Loading("Plates & dumbbells")
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
        ProgramSection.Loading -> ProgramSectionScaffold(modifier) { LoadingSection(state, vm) }
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
    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Spacer(Modifier.height(16.dp))
        // Section nav as a compact pill row at the top (was a tall vertical nav-row list). Each pill
        // drills into that section; current values live inside, so the menu itself stays short.
        FlowRow(
            modifier = Modifier.padding(horizontal = 24.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            ProgramSection.entries.forEach { s ->
                SettingsOutlineAction(s.title) { onOpen(s) }
            }
            SettingsOutlineAction("Build / edit plan") { onOpenBuilder() }
        }
        // The auto-refresh setting sits with the config; the action buttons (Generate, manual
        // re-roll/deload) group together at the very END of the page, never mid-scroll.
        AutoRefreshBlock(state, vm)
        GenerateBlock(state, vm)
        ManualRefreshBlock(vm)
        Spacer(Modifier.height(24.dp))
    }
}

// ─── Sections ──────────────────────────────────────────────────────────────────

@Composable
private fun PlanSection(state: SettingsUiState, vm: SettingsViewModel) {
    ProgramBlock("Training mode", "Follow a fixed plan, or log freely each session from Home.") {
        ChipFlow {
            PillChip("Follow a plan", !state.freestyleMode) { vm.setFreestyleMode(false) }
            PillChip("Go with the flow", state.freestyleMode) { vm.setFreestyleMode(true) }
        }
    }
    ProgramBlock("Training days per week", "How many days you train; the split adapts to the count.") {
        ChipFlow { (1..7).forEach { n -> PillChip("$n", state.daysPerWeek == n) { vm.setDaysPerWeek(n) } } }
    }
    ScheduleBlock(vm)
}

/** Day-aware scheduling: choose sequence vs a fixed weekly plan, and (when weekday) assign each day. */
@Composable
private fun ScheduleBlock(vm: SettingsViewModel) {
    val mode by vm.scheduleMode.collectAsState()
    val schedule by vm.weeklySchedule.collectAsState()
    ProgramBlock("Schedule", "By weekday pins each day to a workout; in sequence gives the next day.") {
        ChipFlow {
            PillChip("In sequence", mode == WeeklySchedule.MODE_SEQUENCE) {
                vm.setScheduleMode(WeeklySchedule.MODE_SEQUENCE)
            }
            PillChip("By weekday", mode == WeeklySchedule.MODE_WEEKDAY) {
                vm.setScheduleMode(WeeklySchedule.MODE_WEEKDAY)
            }
        }
    }
    if (mode == WeeklySchedule.MODE_WEEKDAY) {
        val days = com.forge.app.program.Program.days
        val weekdays = listOf("Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun")
        ProgramBlock("Weekly plan", "Assign a workout to each day, or Rest.") {
            Column(
                Modifier.padding(horizontal = 24.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                weekdays.forEachIndexed { wd, label ->
                    val assigned = schedule.getOrElse(wd) { "" }
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Text(
                            label,
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.width(34.dp)
                        )
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            PillChip("Rest", assigned.isBlank()) { vm.setScheduleDay(wd, "") }
                            days.forEach { d ->
                                PillChip(d.defaultName, assigned == d.key) { vm.setScheduleDay(wd, d.key) }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun GoalExperienceSection(state: SettingsUiState, vm: SettingsViewModel) {
    ProgramBlock("Goal", "Shapes your rep ranges around what you're training for.") {
        ChipFlow {
            listOf(
                "build_muscle" to "Build muscle", "get_stronger" to "Get stronger",
                "lose_weight" to "Lose weight", "general_fitness" to "General fitness"
            ).forEach { (value, label) -> PillChip(label, state.userGoal == value) { vm.setUserGoal(value) } }
        }
    }
    ProgramBlock("Experience", "Sets your volume and which movements you're given.") {
        ChipFlow {
            listOf("beginner" to "Beginner", "intermediate" to "Intermediate", "advanced" to "Advanced")
                .forEach { (value, label) -> PillChip(label, state.experience == value) { vm.setExperience(value) } }
        }
    }
}

@Composable
private fun EmphasisSection(state: SettingsUiState, vm: SettingsViewModel) {
    ProgramBlock("Emphasis", "A one-tap volume lean toward a region.") {
        ChipFlow {
            listOf(
                "balanced" to "Balanced", "upper" to "Upper body",
                "legs" to "Legs", "arms-shoulders" to "Arms & shoulders"
            ).forEach { (value, label) -> PillChip(label, state.programEmphasis == value) { vm.setProgramEmphasis(value) } }
        }
    }
    ProgramBlock("Priority muscles", "Push extra volume into specific muscles.") {
        ChipFlow {
            com.forge.app.program.MuscleGroup.entries.forEach { m ->
                PillChip(m.displayName, m.code in state.priorityMuscles) { vm.togglePriorityMuscle(m.code) }
            }
        }
    }
    ProgramBlock("Problem areas", "Flag a sore joint and the generator steers around it.") {
        ChipFlow {
            com.forge.app.program.ProblemArea.entries.forEach { a ->
                PillChip(a.displayName, a.code in state.problemAreas) { vm.toggleProblemArea(a.code) }
            }
        }
    }
}

/**
 * Equipment stays inside Program (the page is "Program & equipment"): the generator only picks
 * movements you can do with what's selected, plus the plate/dumbbell ceilings it loads against.
 */
@Composable
private fun EquipmentSection(state: SettingsUiState, vm: SettingsViewModel) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    // Becomes true once equipment is touched this visit, so the regenerate prompt only nags after a change.
    var equipmentEdited by remember { mutableStateOf(false) }

    ProgramBlock("Available equipment", "Generated plans only pick movements you can do with these.") {
        // Quick presets — one-tap fill. The Developer's preset is curated (a locked exercise pool).
        ChipFlow {
            com.forge.app.program.equipmentPresets.forEach { preset ->
                val selected = state.availableEquipment == preset.equipment &&
                    state.frozenExerciseIds == preset.frozenIds
                PillChip(preset.label, selected) { vm.selectEquipmentPreset(preset); equipmentEdited = true }
            }
        }
        if (state.frozenExerciseIds != null) {
            Spacer(Modifier.height(10.dp))
            Text(
                "Curated preset: its exercise list is locked. Tap any equipment to switch to a custom set.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp)
            )
        }
        Spacer(Modifier.height(12.dp))
        ChipFlow {
            com.forge.app.program.Equipment.entries.forEach { equip ->
                val selected = equip.name in state.availableEquipment
                PillChip(equip.display.uppercase(), selected) {
                    val current = state.availableEquipment.toMutableSet()
                    if (selected) current.remove(equip.name) else current.add(equip.name)
                    vm.setAvailableEquipment(current)
                    equipmentEdited = true
                }
            }
        }
        Spacer(Modifier.height(16.dp))
        Text(
            if (equipmentEdited) "Equipment changed. Regenerate so your plan uses only what you've got."
            else "Changed your equipment? Regenerate so the plan matches it.",
            style = MaterialTheme.typography.bodySmall,
            color = if (equipmentEdited) accent else muted,
            fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(horizontal = 24.dp)
        )
        Spacer(Modifier.height(10.dp))
        ChipFlow {
            SettingsPrimaryAction("Regenerate for this equipment") {
                vm.generateProgram(state.daysPerWeek); equipmentEdited = false
            }
        }
    }
}

/** How your equipment loads — plate weight for plate-loaded machines, and the dumbbell ceiling. */
@Composable
private fun LoadingSection(state: SettingsUiState, vm: SettingsViewModel) {
    ProgramBlock("Weight per plate", "What one plate weighs, for plate-loaded machines.") {
        ChipFlow {
            listOf(5.0, 10.0, 15.0, 20.0, 25.0, 45.0).forEach { w ->
                PillChip(com.forge.app.domain.units.formatWeight(w, state.useKg), state.plateWeightLb == w) { vm.setPlateWeightLb(w) }
            }
        }
    }

    ProgramBlock("Heaviest dumbbell", "If your dumbbells cap out, heavy lifts switch to reps or the plate stack.") {
        ChipFlow {
            PillChip("No limit", state.maxDbWeightLb == null) { vm.setMaxDbWeightLb(null) }
            listOf(15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 75.0, 100.0).forEach { w ->
                PillChip(com.forge.app.domain.units.formatWeight(w, state.useKg), state.maxDbWeightLb == w) { vm.setMaxDbWeightLb(w) }
            }
        }
    }
}

// ─── Generate / refresh cluster (menu root) ──────────────────────────────────

@Composable
private fun GenerateBlock(state: SettingsUiState, vm: SettingsViewModel) {
    ProgramBlock("Generate", "Builds a fresh plan from your settings above, replacing the current one.") {
        ChipFlow {
            SettingsPrimaryAction("Generate ${state.daysPerWeek}-day plan") { vm.generateProgram(state.daysPerWeek) }
        }
    }
}

/** Auto re-roll the exercises on a cadence. */
@Composable
private fun AutoRefreshBlock(state: SettingsUiState, vm: SettingsViewModel) {
    ProgramBlock("Auto-refresh", "Re-roll the exercises automatically after this many finished sessions.") {
        ChipFlow {
            PillChip("Never", state.rotationCadence == "never") { vm.setRotationCadence("never", state.rotationEveryN) }
            listOf(4, 8, 12).forEach { n ->
                PillChip("Every $n", state.rotationCadence == "every_n" && state.rotationEveryN == n) {
                    vm.setRotationCadence("every_n", n)
                }
            }
        }
    }
}

/** One-tap refreshes beside Generate: swap exercises now, or drop into a recovery week. */
@Composable
private fun ManualRefreshBlock(vm: SettingsViewModel) {
    ProgramBlock("Manual refresh", "Re-roll swaps the exercises; deload rebuilds the week at half volume.") {
        ChipFlow {
            SettingsOutlineAction("Re-roll exercises") { vm.rerollProgram() }
            SettingsOutlineAction("Deload week") { vm.generateDeloadWeek() }
        }
    }
}

// ─── Shared layout helpers ───────────────────────────────────────────────────

/** A plain scrolling container for a Program sub-section. Back is the top-bar `←` alone (one per
 *  page, §2); the top bar's right label already names the section, so there's no in-page back row. */
@Composable
private fun ProgramSectionScaffold(modifier: Modifier, content: @Composable () -> Unit) {
    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        content()
        Spacer(Modifier.height(24.dp))
    }
}

/** Mono anchor + one trimmed caption + a row of controls — the repeated shape of every settings block. */
@Composable
internal fun ProgramBlock(title: String, desc: String, content: @Composable () -> Unit) {
    SettingsSectionHeader(title, top = 22.dp)
    Text(
        desc,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        fontStyle = FontStyle.Italic,
        modifier = Modifier.padding(horizontal = 24.dp)
    )
    Spacer(Modifier.height(10.dp))
    content()
}

@Composable
internal fun ChipFlow(content: @Composable FlowRowScope.() -> Unit) {
    FlowRow(
        modifier = Modifier.padding(horizontal = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        content = content
    )
}

