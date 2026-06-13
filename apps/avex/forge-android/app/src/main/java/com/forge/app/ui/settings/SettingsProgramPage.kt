@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.activity.compose.BackHandler
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

/**
 * The Program settings, split into a small menu of focused sub-pages so the page isn't one
 * long crowded scroll (was ~10 stacked sections). The menu lists each sub-page with its
 * current value; tapping drills in. The primary "Generate" action stays on the menu root,
 * always one tap away. Nested navigation is local state (mirrors the Exercise-likes page),
 * so the outer Settings nav is untouched.
 */
private enum class ProgramSection(val title: String) {
    Split("Split & schedule"),
    Goal("Goal & experience"),
    Emphasis("Emphasis & priorities"),
    Maintenance("Auto-refresh & cardio"),
    Coach("Coach")
}

@Composable
internal fun ProgramPage(
    state: SettingsUiState,
    vm: SettingsViewModel,
    modifier: Modifier = Modifier,
    onOpenCoachBrief: () -> Unit = {}
) {
    var section by remember { mutableStateOf<ProgramSection?>(null) }
    BackHandler(enabled = section != null) { section = null }

    when (section) {
        null -> ProgramMenu(state, vm, modifier) { section = it }
        ProgramSection.Split -> ProgramSectionScaffold(ProgramSection.Split, modifier, onBack = { section = null }) {
            SplitSection(state, vm)
        }
        ProgramSection.Goal -> ProgramSectionScaffold(ProgramSection.Goal, modifier, onBack = { section = null }) {
            GoalSection(state, vm)
        }
        ProgramSection.Emphasis -> ProgramSectionScaffold(ProgramSection.Emphasis, modifier, onBack = { section = null }) {
            EmphasisSection(state, vm)
        }
        ProgramSection.Maintenance -> ProgramSectionScaffold(ProgramSection.Maintenance, modifier, onBack = { section = null }) {
            MaintenanceSection(state, vm)
        }
        ProgramSection.Coach -> ProgramSectionScaffold(ProgramSection.Coach, modifier, onBack = { section = null }) {
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
    onOpen: (ProgramSection) -> Unit
) {
    val priorityCount = state.priorityMuscles.size
    val subtitles = mapOf(
        ProgramSection.Split to "${state.daysPerWeek} days/week",
        ProgramSection.Goal to "${goalLabel(state.userGoal)} · ${state.experience.replaceFirstChar { it.uppercase() }}",
        ProgramSection.Emphasis to "${emphasisLabel(state.programEmphasis)}" +
            if (priorityCount > 0) " · $priorityCount priority" else "",
        ProgramSection.Maintenance to (if (state.rotationCadence == "never") "Auto-refresh off" else "Every ${state.rotationEveryN}") +
            (if (state.cardioWeeklyTargetMin > 0) " · cardio ${state.cardioWeeklyTargetMin}m" else ""),
        ProgramSection.Coach to if (state.coachMode == "auto") "Earning auto-apply" else "Suggest mode"
    )
    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Spacer(Modifier.height(8.dp))
        ProgramSection.entries.forEach { s ->
            SettingsNavRow(s.title, subtitles[s] ?: "") { onOpen(s) }
            SectionDivider()
        }
        Spacer(Modifier.height(16.dp))
        GenerateBlock(state, vm)
        Spacer(Modifier.height(16.dp))
        SectionDivider()
    }
}

// ─── Sections ──────────────────────────────────────────────────────────────────

@Composable
private fun SplitSection(state: SettingsUiState, vm: SettingsViewModel) {
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
    ProgramBlock("Problem areas", "Flag a sore joint and the generator steers around movements that stress it.") {
        ChipFlow {
            com.forge.app.program.ProblemArea.entries.forEach { a ->
                PillChip(a.displayName, a.code in state.problemAreas) { vm.toggleProblemArea(a.code) }
            }
        }
    }
    SectionDivider()
}

@Composable
private fun MaintenanceSection(state: SettingsUiState, vm: SettingsViewModel) {
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
    ProgramBlock("Manual refresh", "Re-roll keeps your split and swaps the exercises. Deload rebuilds the week at ~half volume for recovery.") {
        ChipFlow {
            PillChip("Re-roll exercises now", selected = false) { vm.rerollProgram() }
            PillChip("Deload week", selected = false) { vm.generateDeloadWeek() }
        }
    }
    ProgramBlock("Weekly cardio goal", "A weekly cardio target tracked on the home screen. Log cardio in the Cardio tab whenever you do it.") {
        ChipFlow {
            PillChip("Off", state.cardioWeeklyTargetMin == 0) { vm.setCardioWeeklyTargetMin(0) }
            listOf(60, 120, 150, 200).forEach { m ->
                PillChip("$m min", state.cardioWeeklyTargetMin == m) { vm.setCardioWeeklyTargetMin(m) }
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
    LaunchedEffect(Unit) { vm.loadCoachData() }

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
                Row(
                    Modifier.fillMaxWidth().padding(vertical = 4.dp),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(t.label, style = MaterialTheme.typography.bodySmall, color = onBg)
                    Text(
                        if (t.earned) "auto ✓" else "${t.streak}/${t.required} accepted",
                        style = MaterialTheme.typography.labelSmall,
                        color = if (t.earned) MaterialTheme.colorScheme.primary else muted
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
                        Text(
                            "${d.summary} · ${d.status}",
                            style = MaterialTheme.typography.bodySmall, color = muted,
                            modifier = Modifier.weight(1f)
                        )
                        if (d.status == "applied" && d.undoData != null) {
                            Text(
                                "undo",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.primary,
                                modifier = Modifier
                                    .clickable { vm.undoCoachDecision(d.id) }
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
