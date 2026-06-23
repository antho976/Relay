@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.onboarding

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.fromDisplayWeight
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.program.Equipment
import com.forge.app.program.EquipmentPreset
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.GeneratedDay
import com.forge.app.program.ProblemArea
import com.forge.app.program.equipmentPresets
import kotlin.math.roundToInt

/** Goal options with a one-line explanation of what each one changes (it only reshapes rep ranges). */
private val GOAL_DETAILS = listOf(
    Triple("build_muscle", "Build muscle", "Moderate reps (≈8–12). Balanced for size — the default."),
    Triple("get_stronger", "Get stronger", "Heavier, lower reps (≈4–6 on the big lifts). Strength first."),
    Triple("lose_weight", "Lose weight", "Higher reps (≈12–20) with more conditioning."),
    Triple("general_fitness", "General fitness", "Balanced moderate reps for all-round training.")
)

/**
 * Experience in plain language, mapped to the generator's level keys. The bands are non-overlapping
 * so the choice is unambiguous (the old "0–6 months" / "a few months in" pair overlapped).
 */
private val EXPERIENCE_DETAILS = listOf(
    Triple("beginner", "New to lifting", "Under 6 months. A bit less volume, and no advanced lifts yet."),
    Triple("intermediate", "Got the basics down", "About 6 months to 2 years. Standard volume."),
    Triple("advanced", "Experienced", "2+ years of consistent training. A touch more volume.")
)

/**
 * Sex — only scales the bodyweight-relative strength standards on the Stats tab. Fully optional, so
 * an explicit "Prefer not to say" (stored as "") lets the user opt out instead of just skipping.
 */
private val SEX_DETAILS = listOf(
    Triple("male", "Male", null),
    Triple("female", "Female", null),
    Triple("", "Prefer not to say", null)
)

/** Plan source — generated, self-built in the editor, or no plan at all (freestyle logging). */
private val PLAN_MODE_DETAILS = listOf(
    Triple("generated", "Build me a plan", "Forge picks your exercises from your gear and goal. Recommended."),
    Triple("custom", "I'll make my own", "Skip the questions and build your own plan in the editor."),
    Triple("freestyle", "Go with the flow", "No fixed plan — just log what you did at the gym, whenever you want.")
)

@Composable
internal fun StepPlanMode(selected: String, onSelect: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Headline("How do you want to train?")
        Caption("You can change this later in Settings — nothing here is permanent.")
        PLAN_MODE_DETAILS.forEach { (key, label, desc) -> SelectableRow(label, selected == key, desc) { onSelect(key) } }
    }
}

/** Final step of the generated path: the live generated week with a re-roll. The exact week shown is
 *  what gets saved (edits live in the Program Editor, not here). */
@Composable
internal fun StepPreview(days: List<GeneratedDay>, onRegenerate: () -> Unit) {
    Column(
        modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Headline("Here's your week")
        Caption("Built from your answers. Tap re-roll for a different set of moves, or start training. You can fine-tune it anytime in the editor.")
        OnboardingChip("↻ Re-roll", selected = false, onClick = onRegenerate)
        Spacer(Modifier.height(4.dp))
        days.forEach { day ->
            Column(
                modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp))
                    .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)).padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                Text(
                    "${day.name} · ${day.exercises.size} exercises",
                    style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface
                )
                val names = day.exercises.joinToString(" · ") { ex ->
                    ExerciseLibrary.byId(ex.libId)?.name ?: ex.libId
                }.ifBlank { "Cardio" }
                Text(names, style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

@Composable
internal fun StepName(name: String, onNameChange: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("FORGE", style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.Bold)
        Headline("What do you go by?")
        Caption("Optional — leave it blank and tap Next to skip. Used only for the greeting.")
        OutlinedTextField(
            value = name, onValueChange = onNameChange,
            modifier = Modifier.fillMaxWidth(),
            placeholder = { Text("Your name") },
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
            singleLine = true
        )
    }
}

@Composable
internal fun StepUnits(useKg: Boolean, onToggle: (Boolean) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Headline("Weight units")
        Caption("All weights in the app use this unit. You can change it later in Settings.")
        Row(
            modifier = Modifier.fillMaxWidth()
                .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f), RoundedCornerShape(12.dp))
                .padding(horizontal = 14.dp, vertical = 12.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(if (useKg) "Kilograms (kg)" else "Pounds (lb)",
                style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onSurface)
            Switch(checked = useKg, onCheckedChange = onToggle)
        }
    }
}

@Composable
internal fun StepGoal(selected: String, onSelect: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Headline("What's your main goal?")
        Caption("Same exercises, different loading — this only changes your rep ranges. Switch it anytime.")
        GOAL_DETAILS.forEach { (key, label, desc) -> SelectableRow(label, selected == key, desc) { onSelect(key) } }
    }
}

@Composable
internal fun StepExperience(selected: String, onSelect: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Headline("How long have you been training?")
        Caption("Sets your training volume and which movements you're given.")
        EXPERIENCE_DETAILS.forEach { (key, label, desc) -> SelectableRow(label, selected == key, desc) { onSelect(key) } }
    }
}

@Composable
internal fun StepSex(selected: String?, onSelect: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Headline("Sex")
        Caption("Only used to scale the bodyweight-relative strength standards on the Stats tab. Optional.")
        SEX_DETAILS.forEach { (key, label, desc) -> SelectableRow(label, selected == key, desc) { onSelect(key) } }
    }
}

@Composable
internal fun StepBodyweight(input: String, useKg: Boolean, onInputChange: (String) -> Unit) {
    val unitLabel = if (useKg) "kg" else "lb"
    // A typed value that's blank-or-valid lets "Next" proceed; anything outside the plausible range
    // would otherwise just disable Next with no hint, leaving the user stuck. Show why instead.
    val invalid = input.isNotBlank() && parseSaneBodyweightLb(input, useKg) == null
    val minDisp = toDisplayWeight(MIN_BODYWEIGHT_LB, useKg).roundToInt()
    val maxDisp = toDisplayWeight(MAX_BODYWEIGHT_LB, useKg).roundToInt()
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Headline("Starting bodyweight")
        Caption("Optional — used for relative strength comparisons.")
        OutlinedTextField(
            value = input, onValueChange = onInputChange,
            modifier = Modifier.fillMaxWidth(),
            placeholder = { Text("e.g. 170") },
            suffix = { Text(unitLabel) },
            isError = invalid,
            supportingText = if (invalid) {
                { Text("Enter a weight between $minDisp and $maxDisp $unitLabel, or leave it blank.") }
            } else null,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
            singleLine = true
        )
    }
}

@Composable
internal fun StepDays(days: Int, onChange: (Int) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Headline("Days per week")
        Caption("Your split adapts to this — 3 = Push/Pull/Legs, 4 = Upper/Lower, 7 adds an arms day. It's also your weekly target on the home screen.")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            (1..7).forEach { n -> OnboardingChip("$n", days == n) { onChange(n) } }
        }
    }
}

@Composable
internal fun StepEquipment(
    selected: Set<String>,
    frozenIds: Set<String>?,
    onToggle: (String) -> Unit,
    onSelectPreset: (EquipmentPreset) -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Headline("Your equipment")
        Caption("Pick a preset, then fine-tune. Generated plans only use what's on.")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            equipmentPresets.forEach { preset ->
                val isSel = selected == preset.equipment && frozenIds == preset.frozenIds
                OnboardingChip(preset.label, isSel) { onSelectPreset(preset) }
            }
        }
        Spacer(Modifier.height(4.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Equipment.entries.forEach { e -> OnboardingChip(e.display, e.name in selected) { onToggle(e.name) } }
        }
    }
}

@Composable
internal fun StepPlateWeight(plateWeightLb: Double, useKg: Boolean, onSet: (Double) -> Unit) {
    // Plate denominations in the user's OWN unit — a kg lifter shouldn't have to translate lb plates
    // in their head. The value is always stored in lb (onSet); kg chips convert on the way in via the
    // shared converter so the factor can't drift.
    val plates = if (useKg) listOf(1.25, 2.5, 5.0, 10.0, 15.0, 20.0, 25.0) else listOf(5.0, 10.0, 15.0, 20.0, 25.0, 45.0)
    val unit = if (useKg) "kg" else "lb"
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Headline("Weight per plate")
        Caption("Only matters if your machine is loaded by counting plates (not a numbered stack). This is what one of those plates weighs. Not sure? Just leave the default — you can change it later.")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            plates.forEach { value ->
                val lb = fromDisplayWeight(value, useKg)
                val label = if (value % 1.0 == 0.0) "${value.toInt()} $unit" else "$value $unit"
                OnboardingChip(label, kotlin.math.abs(plateWeightLb - lb) < 0.05) { onSet(lb) }
            }
        }
    }
}

@Composable
internal fun StepProblemAreas(selected: Set<String>, onToggle: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Headline("Any problem areas?")
        Caption("Flag a sore joint and the generator steers around movements that stress it. Optional.")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            ProblemArea.entries.forEach { a -> OnboardingChip(a.displayName, a.code in selected) { onToggle(a.code) } }
        }
    }
}

@Composable
internal fun StepCadence(cadence: String, everyN: Int, onSet: (String, Int) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Headline("Auto-refresh your plan?")
        Caption("Re-roll your exercises automatically after this many workouts — same split, fresh moves. Change it anytime.")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            OnboardingChip("Never", cadence == "never") { onSet("never", everyN) }
            listOf(4, 8, 12).forEach { n ->
                OnboardingChip("Every $n", cadence == "every_n" && everyN == n) { onSet("every_n", n) }
            }
        }
    }
}
