@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.onboarding

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.units.fromDisplayWeight
import com.forge.app.program.Equipment
import com.forge.app.program.EquipmentPreset
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.GeneratedDay
import com.forge.app.program.ProblemArea
import com.forge.app.program.equipmentGroups
import com.forge.app.program.equipmentPresets
import com.forge.app.ui.common.parseAccentHex
import com.forge.app.ui.theme.ForgeMotion

/** Steps 6–11 of the generated path: training days, gym gear, plates, tuning, and the week preview. */

/** What split each day-count builds (SplitTemplates.forDays) — read out live under the day chips. */
private val SPLIT_NAMES = mapOf(
    1 to "Full body",
    2 to "Full body A · B",
    3 to "Push · Pull · Legs",
    4 to "Upper · Lower, twice",
    5 to "Push · Pull · Legs + Upper · Lower",
    6 to "Push · Pull · Legs, twice",
    7 to "Push · Pull · Legs, twice + arms day"
)

@Composable
internal fun StepDays(days: Int, onChange: (Int) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepEyebrow("Your gym")
        StepTitle("How many days a week?")
        StepCaption("Your split adapts, and it becomes your weekly target on Home.")
        Spacer(Modifier.height(2.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            (1..7).forEach { n -> DayChip(n, days == n) { onChange(n) } }
        }
        // Live split readout — the choice answers with what it builds.
        Text(
            if (days in 1..7) SPLIT_NAMES.getValue(days).uppercase() else "PICK YOUR DAYS",
            style = MaterialTheme.typography.labelMedium,
            color = if (days in 1..7) MaterialTheme.colorScheme.primary
            else MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
        )
    }
}

/** GYMAP-20 step 1 of 2: pick the closest gym preset; the selection answers with what's in it. */
@Composable
internal fun StepGymPresets(
    selected: Set<String>,
    frozenIds: Set<String>?,
    onSelectPreset: (EquipmentPreset) -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepEyebrow("Your gym")
        StepTitle("What's in your gym?")
        StepCaption("Pick the closest setup. You fine-tune every piece next.")
        Spacer(Modifier.height(2.dp))
        equipmentPresets.chunked(2).forEach { rowPresets ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                rowPresets.forEach { preset ->
                    PresetTile(
                        icon = OnboardingIcons.forPreset(preset.id),
                        label = preset.label,
                        meta = presetMeta(preset),
                        selected = selected == preset.equipment && frozenIds == preset.frozenIds,
                        onClick = { onSelectPreset(preset) },
                        modifier = Modifier.weight(1f)
                    )
                }
                if (rowPresets.size == 1) Spacer(Modifier.weight(1f))
            }
        }
        // "What's in it" — the live gear readout of the current selection (preset or hand-tuned).
        if (selected.isNotEmpty()) {
            Spacer(Modifier.height(4.dp))
            StepSectionLabel("In this setup", meta = "${selected.size} on")
            Text(
                gearReadout(selected),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                lineHeight = 16.sp,
                maxLines = 4,
                overflow = TextOverflow.Ellipsis
            )
        }
    }
}

/** GYMAP-20 step 2 of 2: every piece of gear, grouped, toggleable. */
@Composable
internal fun StepFineTune(selected: Set<String>, onToggle: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepEyebrow("Your gym")
        StepTitle("Fine-tune your gear")
        StepCaption("Toggle anything the preset got wrong. Plans only use what's on.")
        Spacer(Modifier.height(2.dp))
        equipmentGroups.forEach { (group, items) ->
            StepSectionLabel(group, meta = "${items.count { it.name in selected }} on")
            items.chunked(3).forEach { rowGear ->
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    rowGear.forEach { e ->
                        EquipmentTile(
                            icon = OnboardingIcons.forEquipment(e),
                            label = e.display,
                            selected = e.name in selected,
                            onClick = { onToggle(e.name) },
                            modifier = Modifier.weight(1f)
                        )
                    }
                    repeat(3 - rowGear.size) { Spacer(Modifier.weight(1f)) }
                }
            }
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

/** The selection's gear, in enum order — or one line when it's simply everything. */
private fun gearReadout(selected: Set<String>): String =
    if (selected.size == Equipment.entries.size) "EVERY PIECE ON THE FINE-TUNE LIST"
    else Equipment.entries.filter { it.name in selected }
        .joinToString(" · ") { it.display }.uppercase()

@Composable
internal fun StepPlateWeight(plateWeightLb: Double, useKg: Boolean, onSet: (Double) -> Unit) {
    // Plate denominations in the user's OWN unit — a kg lifter shouldn't translate lb plates in
    // their head. Stored in lb (onSet); kg chips convert on the way in via the shared converter.
    val plates = if (useKg) listOf(1.25, 2.5, 5.0, 10.0, 15.0, 20.0, 25.0) else listOf(5.0, 10.0, 15.0, 20.0, 25.0, 45.0)
    val unit = if (useKg) "kg" else "lb"
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepEyebrow("Your gym")
        StepTitle("Weight per plate")
        StepCaption("For machines loaded by counting plates. Not sure? Keep the default.")
        Spacer(Modifier.height(2.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            plates.forEach { value ->
                val lb = fromDisplayWeight(value, useKg)
                val label = if (value % 1.0 == 0.0) "${value.toInt()} $unit" else "$value $unit"
                ChoiceChip(label, kotlin.math.abs(plateWeightLb - lb) < 0.05, { onSet(lb) })
            }
        }
    }
}

@Composable
internal fun StepProblemAreas(selected: Set<String>, onToggle: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepEyebrow("Fine-tuning")
        StepTitle("Any problem areas?")
        StepCaption("Flag a sore joint or old injury and your plan eases off it. Optional.")
        Spacer(Modifier.height(2.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            ProblemArea.entries.forEach { a ->
                ChoiceChip(a.displayName, a.code in selected, { onToggle(a.code) })
            }
        }
    }
}

@Composable
internal fun StepCadence(cadence: String, everyN: Int, onSet: (String, Int) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepEyebrow("Fine-tuning")
        StepTitle("Auto-refresh your plan?")
        StepCaption("Re-rolls your exercises after this many workouts. Same split, fresh moves.")
        Spacer(Modifier.height(2.dp))
        OptionCard(
            label = "Never",
            description = "Your plan stays exactly as generated.",
            selected = cadence == "never",
            onClick = { onSet("never", everyN) }
        )
        listOf(4, 8, 12).forEach { n ->
            OptionCard(
                label = "Every $n workouts",
                selected = cadence == "every_n" && everyN == n,
                onClick = { onSet("every_n", n) }
            )
        }
    }
}

/** Final step of the generated path: the live generated week. The exact week shown is what gets
 *  saved (the re-roll sits beside "Let's go" in the bottom bar; edits live in the Program Editor). */
@Composable
internal fun StepPreview(days: List<GeneratedDay>) {
    Column(
        modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        StepEyebrow("Your week")
        StepTitle("Here's your week")
        StepCaption("Built from your answers. Fine-tune it anytime in the editor.")
        Spacer(Modifier.height(2.dp))
        // Re-rolls crossfade the whole list so a new week reads as a fresh deal, not a flicker.
        AnimatedContent(
            targetState = days,
            transitionSpec = { fadeIn(ForgeMotion.enterTween()) togetherWith fadeOut(ForgeMotion.exitTween()) },
            label = "preview_reroll"
        ) { shownDays ->
            Column(verticalArrangement = Arrangement.spacedBy(24.dp)) {
                shownDays.forEach { day -> PreviewDay(day) }
            }
        }
    }
}

/** One day in the program screen's section formula (GYMAP-21/28 — the two render identically):
 *  colour-dot + mono anchor with its set count as right meta, then hang-indented exercise rows with
 *  the sets × reps at the quiet caption rung, so the movement names carry the row. */
@Composable
private fun PreviewDay(day: GeneratedDay) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(Modifier.size(8.dp).clip(CircleShape).background(parseAccentHex(day.accentHex)))
                Text(
                    day.name.uppercase(),
                    style = MaterialTheme.typography.labelLarge,
                    color = muted,
                    letterSpacing = 1.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
            Text(
                "${day.exercises.sumOf { it.sets }} SETS",
                style = MaterialTheme.typography.labelSmall,
                color = muted
            )
        }
        Spacer(Modifier.height(10.dp))
        if (day.exercises.isEmpty()) {
            Text("Cardio", style = MaterialTheme.typography.bodyMedium, color = muted,
                modifier = Modifier.padding(start = 16.dp))
        }
        day.exercises.forEach { ex ->
            Row(
                Modifier.fillMaxWidth().padding(start = 16.dp, top = 5.dp, bottom = 5.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    ExerciseLibrary.byId(ex.libId)?.name ?: ex.libId,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onBackground,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false)
                )
                Spacer(Modifier.width(12.dp))
                Text(
                    "${ex.sets} × ${ex.reps}",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.7f)
                )
            }
        }
    }
}
