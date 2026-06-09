@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.onboarding

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.theme.AccentEmphasis
import com.forge.app.program.Equipment
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.GeneratedDay
import com.forge.app.program.ProblemArea

/** Goal options with a one-line explanation of what each one changes (it only reshapes rep ranges). */
private val GOAL_DETAILS = listOf(
    Triple("build_muscle", "Build muscle", "Moderate reps (≈8–12). Balanced for size — the default."),
    Triple("get_stronger", "Get stronger", "Heavier, lower reps (≈4–6 on the big lifts). Strength first."),
    Triple("lose_weight", "Lose weight", "Higher reps (≈12–20) with more conditioning."),
    Triple("general_fitness", "General fitness", "Balanced moderate reps for all-round training.")
)

/** Experience in plain language, mapped to the generator's level keys. */
private val EXPERIENCE_DETAILS = listOf(
    Triple("beginner", "I just started", "0–6 months in. A bit less volume, and no advanced lifts yet."),
    Triple("intermediate", "A few months in", "Comfortable with the basics. Standard volume."),
    Triple("advanced", "A few years in", "Experienced lifter. A touch more volume.")
)

/** Equipment presets — quick-fill the set instead of toggling nine chips (shared with Settings). */
private val EQUIPMENT_PRESETS: List<Pair<String, Set<String>>> = com.forge.app.program.equipmentPresets

@Composable
private fun Headline(text: String) {
    Text(text, style = MaterialTheme.typography.headlineLarge, fontWeight = FontWeight.Black,
        lineHeight = 38.sp, color = MaterialTheme.colorScheme.onBackground)
}

@Composable
private fun Caption(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant)
}

@Composable
internal fun StepName(name: String, onNameChange: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Text("FORGE", style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.Black)
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
internal fun StepEmphasis(selected: String, onSelect: (String) -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val level = AccentEmphasis.from(selected)
    // Live preview of the picked level applied to a sample stat block.
    val previewColor = when (level) {
        AccentEmphasis.OFF -> onBg
        AccentEmphasis.SUBTLE -> lerp(onBg, accent, 0.45f)
        else -> accent
    }
    val previewWeight = when (level) {
        AccentEmphasis.MEDIUM -> FontWeight.SemiBold
        AccentEmphasis.STRONG -> FontWeight.Bold
        else -> FontWeight.Normal
    }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Headline("Color the important text?")
        Caption("Tints key numbers, titles and names with your accent. Change it anytime in Settings → Appearance.")
        Column(
            modifier = Modifier.fillMaxWidth()
                .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp)
        ) {
            Text("THIS WEEK", style = MaterialTheme.typography.labelSmall, color = muted, letterSpacing = 1.5.sp)
            Text("12,400 lb", style = MaterialTheme.typography.headlineSmall, color = previewColor, fontWeight = previewWeight)
            Text("128 sets · 4 workouts", style = MaterialTheme.typography.bodyMedium, color = muted)
        }
        AccentEmphasis.options.forEach { (key, label) ->
            SelectableRow(label, level == AccentEmphasis.from(key)) { onSelect(key) }
        }
    }
}

@Composable
internal fun StepUnits(useKg: Boolean, onToggle: (Boolean) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Headline("Weight units")
        Caption("All weights in the app use this unit. You can change it later in Settings.")
        Row(
            modifier = Modifier.fillMaxWidth()
                .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
                .padding(16.dp),
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
internal fun StepBodyweight(input: String, useKg: Boolean, onInputChange: (String) -> Unit) {
    val unitLabel = if (useKg) "kg" else "lb"
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Headline("Starting bodyweight")
        Caption("Optional — used for relative strength comparisons.")
        OutlinedTextField(
            value = input, onValueChange = onInputChange,
            modifier = Modifier.fillMaxWidth(),
            placeholder = { Text("e.g. 170") },
            suffix = { Text(unitLabel) },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
            singleLine = true
        )
    }
}

@Composable
internal fun StepDays(days: Int, onChange: (Int) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Headline("Days per week")
        Caption("Your split adapts to this — 3 = Push/Pull/Legs, 4 = Upper/Lower, 7 adds an arms day.")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            (1..7).forEach { n -> OnboardingChip("$n", days == n) { onChange(n) } }
        }
    }
}

@Composable
internal fun StepEquipment(selected: Set<String>, onToggle: (String) -> Unit, onSetAll: (Set<String>) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Headline("Your equipment")
        Caption("Pick a preset, then fine-tune. Generated plans only use what's on.")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            EQUIPMENT_PRESETS.forEach { (label, set) ->
                OnboardingChip(label, selected == set) { onSetAll(set) }
            }
        }
        Spacer(Modifier.height(4.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Equipment.entries.forEach { e -> OnboardingChip(e.display, e.name in selected) { onToggle(e.name) } }
        }
    }
}

@Composable
internal fun StepPlateWeight(plateWeightLb: Double, onSet: (Double) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Headline("Weight per plate")
        Caption("Machine/cable exercises are entered as a plate count. This is what one plate weighs — the MWM-989 is 15 lb. Leave it at 15 if you're not sure.")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf(5.0, 10.0, 15.0, 20.0, 25.0, 45.0).forEach { w ->
                OnboardingChip("${w.toInt()} lb", plateWeightLb == w) { onSet(w) }
            }
        }
    }
}

@Composable
internal fun StepProblemAreas(selected: Set<String>, onToggle: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Headline("Any problem areas?")
        Caption("Flag a sore joint and the generator steers around movements that stress it. Optional.")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            ProblemArea.entries.forEach { a -> OnboardingChip(a.displayName, a.code in selected) { onToggle(a.code) } }
        }
    }
}

@Composable
internal fun StepCadence(cadence: String, everyN: Int, onSet: (String, Int) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
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

/** Final step: the live generated week, with a re-roll. The exact week shown is what gets saved. */
@Composable
internal fun StepPreview(days: List<GeneratedDay>, onRegenerate: () -> Unit) {
    Column(modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Headline("Here's your week")
        Caption("Built from your answers. Tap re-roll for a different set of moves, or start training.")
        OnboardingChip("↻ Re-roll", selected = false, onClick = onRegenerate)
        Spacer(Modifier.height(4.dp))
        days.forEach { day ->
            Column(
                modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp))
                    .background(MaterialTheme.colorScheme.surfaceVariant).padding(14.dp),
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
private fun SelectableRow(label: String, isSelected: Boolean, subtitle: String? = null, onClick: () -> Unit) {
    Box(
        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp))
            .background(
                if (isSelected) MaterialTheme.colorScheme.primary.copy(alpha = 0.15f)
                else MaterialTheme.colorScheme.surfaceVariant
            )
            .clickable { onClick() }.padding(16.dp),
        contentAlignment = Alignment.CenterStart
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(label, style = MaterialTheme.typography.bodyLarge,
                    fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Normal,
                    color = if (isSelected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface)
                if (subtitle != null) Text(subtitle, style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (isSelected) Text("✓", style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.Bold)
        }
    }
}

@Composable
internal fun OnboardingChip(label: String, selected: Boolean, onClick: () -> Unit) {
    Box(
        modifier = Modifier.clip(RoundedCornerShape(50))
            .background(
                if (selected) MaterialTheme.colorScheme.primary.copy(alpha = 0.18f)
                else MaterialTheme.colorScheme.surfaceVariant
            )
            .clickable(onClick = onClick).padding(horizontal = 18.dp, vertical = 10.dp)
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
            color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface)
    }
}
