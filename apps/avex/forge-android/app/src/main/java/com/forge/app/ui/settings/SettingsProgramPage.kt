@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.FlowRowScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import com.forge.app.domain.schedule.WeeklySchedule
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.ui.coach.TrustProgressBar
import com.forge.app.ui.common.clickableLabeled

/**
 * The Program settings, split into a small menu of focused sub-pages so the page isn't one
 * long crowded scroll (was ~10 stacked sections). The menu lists each sub-page with its
 * current value; tapping drills in. The primary "Generate" action stays on the menu root,
 * always one tap away. Nested navigation is local state (mirrors the Exercise-likes page),
 * so the outer Settings nav is untouched. The open section is hoisted to [SettingsScreen] so the
 * top-bar back arrow and system back both return here (the menu) before exiting to Settings.
 */
internal enum class ProgramSection(val title: String) {
    Split("Split & schedule"),
    Goal("Goal & experience"),
    Emphasis("Emphasis & priorities"),
    Equipment("Equipment"),
    Coach("Coach")
}

@Composable
internal fun ProgramPage(
    state: SettingsUiState,
    vm: SettingsViewModel,
    section: ProgramSection?,
    onSectionChange: (ProgramSection?) -> Unit,
    modifier: Modifier = Modifier,
    onOpenCoachBrief: () -> Unit = {},
    onOpenBuilder: () -> Unit = {}
) {
    when (section) {
        null -> ProgramMenu(state, vm, modifier, onOpenBuilder) { onSectionChange(it) }
        ProgramSection.Split -> ProgramSectionScaffold(ProgramSection.Split, modifier, onBack = { onSectionChange(null) }) {
            SplitSection(state, vm)
        }
        ProgramSection.Goal -> ProgramSectionScaffold(ProgramSection.Goal, modifier, onBack = { onSectionChange(null) }) {
            GoalSection(state, vm)
        }
        ProgramSection.Emphasis -> ProgramSectionScaffold(ProgramSection.Emphasis, modifier, onBack = { onSectionChange(null) }) {
            EmphasisSection(state, vm)
        }
        ProgramSection.Equipment -> ProgramSectionScaffold(ProgramSection.Equipment, modifier, onBack = { onSectionChange(null) }) {
            EquipmentSection(state, vm)
        }
        ProgramSection.Coach -> ProgramSectionScaffold(ProgramSection.Coach, modifier, onBack = { onSectionChange(null) }) {
            CoachSection(state, vm, onOpenCoachBrief)
        }
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
    val priorityCount = state.priorityMuscles.size
    val subtitles = mapOf(
        ProgramSection.Split to "${state.daysPerWeek} days/week",
        ProgramSection.Goal to "${goalLabel(state.userGoal)} · ${state.experience.replaceFirstChar { it.uppercase() }}",
        ProgramSection.Emphasis to "${emphasisLabel(state.programEmphasis)}" +
            if (priorityCount > 0) " · $priorityCount priority" else "",
        ProgramSection.Equipment to (if (state.availableEquipment.isEmpty()) "All equipment" else "${state.availableEquipment.size} selected"),
        ProgramSection.Coach to when {
            !state.coachEnabled -> "Off"
            state.coachMode == "auto" -> "Earning auto-apply"
            else -> "Suggest mode"
        }
    )
    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Spacer(Modifier.height(8.dp))
        ProgramSection.entries.forEach { s ->
            SettingsNavRow(s.title, subtitles[s] ?: "") { onOpen(s) }
            SectionDivider()
        }
        SettingsNavRow("Build / edit plan", "Add, rename, reorder or remove days & exercises") { onOpenBuilder() }
        SectionDivider()
        Spacer(Modifier.height(16.dp))
        // The whole "make/refresh a plan" cluster lives together at the bottom: generate fresh,
        // auto-refresh on a cadence, or refresh by hand.
        GenerateBlock(state, vm)
        AutoRefreshBlock(state, vm)
        ManualRefreshBlock(vm)
        Spacer(Modifier.height(16.dp))
        SectionDivider()
    }
}

// ─── Sections ──────────────────────────────────────────────────────────────────

@Composable
private fun SplitSection(state: SettingsUiState, vm: SettingsViewModel) {
    ProgramBlock(
        "Training mode",
        "Follow a plan, or go with the flow — no fixed plan, just log what you trained each session from the home screen."
    ) {
        ChipFlow {
            PillChip("Follow a plan", !state.freestyleMode) { vm.setFreestyleMode(false) }
            PillChip("Go with the flow", state.freestyleMode) { vm.setFreestyleMode(true) }
        }
        Spacer(Modifier.height(8.dp))
        Text(
            "Switching never deletes anything — Go with the flow just hides your plan (it stays saved), " +
                "and your logged workouts remain either way.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 24.dp)
        )
    }
    if (state.weeklyVolume.isNotEmpty()) {
        ProgramBlock("Weekly volume", "Sets per muscle across your current week — tune days & priorities to rebalance.") {
            ChipFlow { state.weeklyVolume.forEach { (name, sets) -> PillChip("$name $sets", selected = false) {} } }
        }
    }
    ProgramBlock(
        "Training days per week",
        "The split adapts to the count — 3 = Push/Pull/Legs, 4 = Upper/Lower, 7 adds an arms day."
    ) {
        ChipFlow { (1..7).forEach { n -> PillChip("$n", state.daysPerWeek == n) { vm.setDaysPerWeek(n) } } }
    }
    ScheduleBlock(vm)
    GenerateBlock(state, vm)
    Spacer(Modifier.height(16.dp))
    SectionDivider()
}

/** Day-aware scheduling: choose sequence vs a fixed weekly plan, and (when weekday) assign each day. */
@Composable
private fun ScheduleBlock(vm: SettingsViewModel) {
    val mode by vm.scheduleMode.collectAsState()
    val schedule by vm.weeklySchedule.collectAsState()
    ProgramBlock(
        "Schedule",
        "By weekday pins each day to a workout (Legs on Monday…) — miss a day and it rolls to the next, " +
            "no catch-up. In sequence just gives the next day after the one you last finished."
    ) {
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
        ProgramBlock("Weekly plan", "Tap a workout for each day, or Rest.") {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
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
                        ChipFlow {
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
private fun GoalSection(state: SettingsUiState, vm: SettingsViewModel) {
    ProgramBlock("Goal", "Shapes your rep ranges — strength trains heavier/lower reps, lose-weight leans higher reps.") {
        ChipFlow {
            listOf(
                "build_muscle" to "Build muscle", "get_stronger" to "Get stronger",
                "lose_weight" to "Lose weight", "general_fitness" to "General fitness"
            ).forEach { (value, label) -> PillChip(label, state.userGoal == value) { vm.setUserGoal(value) } }
        }
    }
    ProgramBlock("Experience", "Sets your volume and which movements you're given — beginners skip the most advanced lifts.") {
        ChipFlow {
            listOf("beginner" to "Beginner", "intermediate" to "Intermediate", "advanced" to "Advanced")
                .forEach { (value, label) -> PillChip(label, state.experience == value) { vm.setExperience(value) } }
        }
    }
    SectionDivider()
}

@Composable
private fun EmphasisSection(state: SettingsUiState, vm: SettingsViewModel) {
    ProgramBlock("Emphasis", "A one-tap volume lean for a whole region. For finer control, use Priority muscles below.") {
        ChipFlow {
            listOf(
                "balanced" to "Balanced", "upper" to "Upper body",
                "legs" to "Legs", "arms-shoulders" to "Arms & shoulders"
            ).forEach { (value, label) -> PillChip(label, state.programEmphasis == value) { vm.setProgramEmphasis(value) } }
        }
    }
    ProgramBlock("Priority muscles", "Pick muscles to push extra volume into. Leave all off for a balanced plan.") {
        ChipFlow {
            com.forge.app.program.MuscleGroup.entries.forEach { m ->
                PillChip(m.displayName, m.code in state.priorityMuscles) { vm.togglePriorityMuscle(m.code) }
            }
        }
    }
    ProgramBlock("Problem areas", "Flag a sore joint or an injury and the generator steers around movements that stress it.") {
        ChipFlow {
            com.forge.app.program.ProblemArea.entries.forEach { a ->
                PillChip(a.displayName, a.code in state.problemAreas) { vm.toggleProblemArea(a.code) }
            }
        }
    }
    SectionDivider()
}

/** Auto re-roll the exercises on a cadence. Lives in the Generate cluster at the bottom of the menu. */
@Composable
private fun AutoRefreshBlock(state: SettingsUiState, vm: SettingsViewModel) {
    ProgramBlock("Auto-refresh", "Automatically re-roll the exercises (same split) after this many finished sessions.") {
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

/** One-tap refreshes that sit beside Generate: swap exercises now, or drop into a recovery week. */
@Composable
private fun ManualRefreshBlock(vm: SettingsViewModel) {
    ProgramBlock("Manual refresh", "Re-roll keeps your split and swaps the exercises. Deload rebuilds the week at ~half volume for recovery.") {
        ChipFlow {
            PillChip("Re-roll exercises now", selected = false) { vm.rerollProgram() }
            PillChip("Deload week", selected = false) { vm.generateDeloadWeek() }
        }
    }
}

/**
 * Equipment lives inside Program now (the page is "Program & equipment"): the generator only picks
 * movements you can do with what's selected here, plus the plate/dumbbell ceilings it loads against.
 */
@Composable
private fun EquipmentSection(state: SettingsUiState, vm: SettingsViewModel) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    // Becomes true once equipment is touched this visit, so the regenerate prompt only nags after a change.
    var equipmentEdited by remember { mutableStateOf(false) }

    ProgramBlock(
        "Available equipment",
        "Generated programs only pick exercises you can do with the equipment you select here."
    ) {
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
                "This is a curated preset — its exercise list is locked and won't change. " +
                    "Tap any equipment below to switch to a custom set.",
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
            if (equipmentEdited) "You changed your equipment — regenerate so your program only uses what you've got."
            else "Changed your equipment? Regenerate so the program matches what you've got.",
            style = MaterialTheme.typography.bodySmall,
            color = if (equipmentEdited) accent else muted,
            fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(horizontal = 24.dp)
        )
        Spacer(Modifier.height(10.dp))
        ChipFlow {
            PillChip("Regenerate for this equipment", selected = equipmentEdited) {
                vm.generateProgram(state.daysPerWeek); equipmentEdited = false
            }
        }
    }

    ProgramBlock(
        "Weight per plate",
        "For machines that load by counting plates (not a numbered stack): exercises are entered and " +
            "shown as a plate count. This is what one plate weighs, used for PRs and volume."
    ) {
        ChipFlow {
            listOf(5.0, 10.0, 15.0, 20.0, 25.0, 45.0).forEach { w ->
                PillChip(com.forge.app.domain.units.formatWeight(w, state.useKg), state.plateWeightLb == w) { vm.setPlateWeightLb(w) }
            }
        }
    }

    ProgramBlock(
        "Heaviest dumbbell",
        "If your dumbbells max out (adjustable sets), heavy lifts in generated programs lean on " +
            "the plate stack instead, and weight suggestions switch to rep progression at the ceiling."
    ) {
        ChipFlow {
            PillChip("No limit", state.maxDbWeightLb == null) { vm.setMaxDbWeightLb(null) }
            listOf(15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 75.0, 100.0).forEach { w ->
                PillChip(com.forge.app.domain.units.formatWeight(w, state.useKg), state.maxDbWeightLb == w) { vm.setMaxDbWeightLb(w) }
            }
        }
    }
    SectionDivider()
}

@Composable
private fun CoachSection(state: SettingsUiState, vm: SettingsViewModel, onOpenCoachBrief: () -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val trust by vm.coachTrust.collectAsState()
    val history by vm.coachHistory.collectAsState()
    val now = remember { System.currentTimeMillis() }
    LaunchedEffect(Unit) { vm.loadCoachData() }

    ProgramBlock(
        "Coach",
        "When on, the coach gets its own tab and suggests weekly tweaks. Turn it off to hide it entirely."
    ) {
        ChipFlow {
            PillChip("On", state.coachEnabled) { vm.setCoachEnabled(true) }
            PillChip("Off", !state.coachEnabled) { vm.setCoachEnabled(false) }
        }
    }
    // Everything below configures the coach — only meaningful while it's enabled.
    if (!state.coachEnabled) {
        SectionDivider()
        return
    }

    ProgramBlock("Week brief", "Your coach's read on the week — last week's numbers, any proposed changes, and a focus.") {
        ChipFlow { PillChip("View this week's brief", selected = false) { onOpenCoachBrief() } }
    }

    ProgramBlock(
        "Coach mode",
        "Suggest: every weekly change waits for your tap in the Week Brief. Earn auto-apply: a change " +
            "type may apply itself ONLY after you've accepted it ${com.forge.app.domain.coach.TrustLedger.CONSERVATIVE_STREAK}–" +
            "${com.forge.app.domain.coach.TrustLedger.AGGRESSIVE_STREAK} weeks in a row — and one bad " +
            "outcome demotes it back. Deloads always ask."
    ) {
        ChipFlow {
            PillChip("Suggest", state.coachMode != "auto") { vm.setCoachMode("suggest") }
            PillChip("Earn auto-apply", state.coachMode == "auto") { vm.setCoachMode("auto") }
        }
        // "Earn auto-apply" is a TARGET, not an on-switch: until a change type builds its accepted
        // streak, nothing self-applies. Say so explicitly so the user never believes they've handed
        // over control prematurely — the trust gate is a core safety property, not a hidden one.
        // trust.isNotEmpty() gates out the load window — coachTrust starts empty, so without it the
        // note would flash on a fully-trusted coach until loadCoachData() populates trust.
        if (state.coachMode == "auto" && trust.isNotEmpty() && trust.none { it.earned }) {
            Spacer(Modifier.height(8.dp))
            Text(
                "Auto-apply isn't active yet — no change type has built the required streak, so every " +
                    "proposal still waits for your tap. It switches on per type as trust is earned below.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.primary,
                fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp)
            )
        }
    }

    ProgramBlock("Earned trust", "What the coach has earned so far, per change type.") {
        Column(Modifier.padding(horizontal = 24.dp)) {
            if (trust.all { it.streak == 0 }) {
                Text(
                    "No accepted proposals yet — trust builds as you apply weekly suggestions.",
                    style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
                )
            }
            trust.forEach { t ->
                Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
                    Row(
                        Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text(t.label, style = MaterialTheme.typography.bodySmall, color = onBg)
                        Text(
                            when {
                                t.earned -> "auto ✓"
                                // Next milestone: how many more accepted weeks unlock auto-apply for this type.
                                else -> "${t.streak}/${t.required} · ${(t.required - t.streak).coerceAtLeast(1)} more to auto-apply"
                            },
                            style = MaterialTheme.typography.labelSmall,
                            color = if (t.earned) MaterialTheme.colorScheme.primary else muted
                        )
                    }
                    Spacer(Modifier.height(6.dp))
                    TrustProgressBar(
                        streak = t.streak, required = t.required, earned = t.earned,
                        modifier = Modifier.fillMaxWidth()
                    )
                }
            }
        }
    }

    ProgramBlock("Coach history", "Every weekly pass, including holds — applied changes can be undone here.") {
        Column(Modifier.padding(horizontal = 24.dp)) {
            if (history.isEmpty()) {
                Text("No passes yet.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
            }
            history.forEach { entry ->
                Text(
                    "${entry.pass.weekId.uppercase()} · ${entry.pass.status.uppercase()}",
                    style = MaterialTheme.typography.labelSmall, color = onBg,
                    modifier = Modifier.padding(top = 10.dp)
                )
                if (entry.decisions.isEmpty()) {
                    entry.pass.holdReason?.let {
                        Text(it, style = MaterialTheme.typography.bodySmall, color = muted)
                    }
                }
                entry.decisions.forEach { d ->
                    Row(
                        Modifier.fillMaxWidth().padding(vertical = 2.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        // Plain-English status: the watcher window/verdict for applied changes, else the raw status.
                        val statusText = com.forge.app.domain.coach.CoachOutcome
                            .label(d.status, d.outcome, d.appliedAt, now) ?: d.status
                        Text(
                            "${d.summary} · $statusText",
                            style = MaterialTheme.typography.bodySmall, color = muted,
                            modifier = Modifier.weight(1f)
                        )
                        if (d.status == "applied" && d.undoData != null) {
                            Text(
                                "undo",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.primary,
                                modifier = Modifier
                                    .clickableLabeled("Undo this change") { vm.undoCoachDecision(d.id) }
                                    .padding(start = 12.dp, top = 2.dp, bottom = 2.dp)
                            )
                        }
                    }
                }
            }
        }
    }
    SectionDivider()
}

@Composable
private fun GenerateBlock(state: SettingsUiState, vm: SettingsViewModel) {
    ProgramBlock(
        "Generate",
        "Builds a fresh program from everything above — equipment, goal, experience, priorities, problem areas & likes/dislikes — replacing the current one."
    ) {
        ChipFlow { PillChip("Generate ${state.daysPerWeek}-day program", selected = false) { vm.generateProgram(state.daysPerWeek) } }
    }
}

// ─── Shared layout helpers ───────────────────────────────────────────────────

/** A section page with an in-page back row (the outer top-bar back exits Program entirely). */
@Composable
private fun ProgramSectionScaffold(
    section: ProgramSection,
    modifier: Modifier,
    onBack: () -> Unit,
    content: @Composable () -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val outline = MaterialTheme.colorScheme.outline
    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Row(
            modifier = Modifier.fillMaxWidth().clickable(onClick = onBack).padding(horizontal = 24.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text("←", style = MaterialTheme.typography.bodyMedium, color = muted)
            Text(section.title.uppercase(), style = MaterialTheme.typography.labelSmall, color = onBg)
        }
        HorizontalDivider(color = outline.copy(alpha = 0.25f), modifier = Modifier.padding(horizontal = 24.dp))
        Spacer(Modifier.height(16.dp))
        content()
        Spacer(Modifier.height(24.dp))
    }
}

/** Title + one-line description + a row of chips — the repeated shape of every Program control. */
@Composable
private fun ProgramBlock(title: String, desc: String, content: @Composable () -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    Text(title, style = MaterialTheme.typography.titleSmall, color = onBg, modifier = Modifier.padding(horizontal = 24.dp))
    Spacer(Modifier.height(4.dp))
    Text(desc, style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic,
        modifier = Modifier.padding(horizontal = 24.dp))
    Spacer(Modifier.height(12.dp))
    content()
    Spacer(Modifier.height(24.dp))
}

@Composable
private fun ChipFlow(content: @Composable FlowRowScope.() -> Unit) {
    FlowRow(
        modifier = Modifier.padding(horizontal = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        content = content
    )
}

private fun goalLabel(code: String) = when (code) {
    "build_muscle" -> "Build muscle"; "get_stronger" -> "Get stronger"
    "lose_weight" -> "Lose weight"; "general_fitness" -> "General fitness"; else -> "Build muscle"
}

private fun emphasisLabel(code: String) = when (code) {
    "upper" -> "Upper body"; "legs" -> "Legs"; "arms-shoulders" -> "Arms & shoulders"; else -> "Balanced"
}
