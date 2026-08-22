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
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.program.DayArchetype
import com.forge.app.program.Equipment
import com.forge.app.program.EquipmentPreset
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.GeneratedDay
import com.forge.app.program.equipmentGroups
import com.forge.app.program.equipmentPresets
import com.forge.app.ui.common.parseAccentHex
import com.forge.app.ui.theme.ForgeMotion

/**
 * The gym half of the generated path — the two steps where the [PlanLedger] under the question stops
 * being an empty shape and starts filling. Picking a preset deals the week; toggling one piece of
 * gear moves the meters while you watch. Then the week page, where the same mark leads the real
 * thing.
 */

/** GYMAP-20 step 1 of 2: pick the closest gym preset. */
@Composable
internal fun StepGymPresets(
    selected: Set<String>,
    frozenIds: Set<String>?,
    onSelectPreset: (EquipmentPreset) -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
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
        // The "in this setup" mono gear dump that used to close this page is gone (2026-08-22): the
        // ledger below now answers what the preset DID to the week, and the fine-tune page next lists
        // every piece with its own on/off state. Two readouts of one answer broke §4.3's one home.
    }
}

/** GYMAP-20 step 2 of 2: every piece of gear, grouped, toggleable. */
@Composable
internal fun StepFineTune(selected: Set<String>, onToggle: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepTitle("Fine-tune your gear")
        StepCaption("Toggle anything the preset got wrong. Plans use only what's on.")
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

/**
 * Last step of the generated path: the week that has been building under every question since the
 * day-count, now at full size. The same [PlanLedger] leads it — the mark the user has been reading
 * for three screens, so arriving here is a zoom, not a reveal of something new. Under it, the days
 * open out into their real exercise lists.
 *
 * The exact week shown is the week that gets saved (the re-roll sits beside the CTA in the bottom
 * bar; edits live in the Program Editor).
 */
@Composable
internal fun StepWeek(
    archetypes: List<DayArchetype>,
    plannedSets: List<Int>,
    days: List<GeneratedDay>
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        StepTitle("Here's your week")
        StepCaption("Built from your answers. Change any of it in the editor.")
        Spacer(Modifier.height(6.dp))
        PlanLedger(archetypes = archetypes, plannedSets = plannedSets, days = days, trackHeight = 88.dp)
        Spacer(Modifier.height(12.dp))
        // Re-rolls crossfade the whole list so a new week reads as a fresh deal, not a flicker.
        AnimatedContent(
            targetState = days,
            transitionSpec = { fadeIn(ForgeMotion.enterTween()) togetherWith fadeOut(ForgeMotion.exitTween()) },
            label = "week_reroll"
        ) { shownDays ->
            Column(verticalArrangement = Arrangement.spacedBy(24.dp)) {
                shownDays.forEach { day -> WeekDay(day) }
            }
        }
    }
}

/** One day in the program screen's section formula (GYMAP-21/28 — the two render identically):
 *  colour-dot + mono anchor with its set count as right meta, then hang-indented exercise rows with
 *  the sets × reps at the quiet caption rung, so the movement names carry the row. */
@Composable
private fun WeekDay(day: GeneratedDay) {
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
                    maxLines = 2,
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
            // Effectively unreachable (the generator keeps a last-resort fill), but a blank day must
            // still say what it means rather than mislabel itself.
            Text(
                "Nothing your gear covers yet",
                style = MaterialTheme.typography.bodyMedium,
                color = muted,
                modifier = Modifier.padding(start = 16.dp)
            )
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
