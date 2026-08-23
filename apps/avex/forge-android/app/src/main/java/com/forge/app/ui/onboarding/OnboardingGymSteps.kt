@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.onboarding

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.program.DayArchetype
import com.forge.app.program.Equipment
import com.forge.app.program.EquipmentPreset
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.GeneratedDay
import com.forge.app.program.equipmentGroups
import com.forge.app.program.equipmentPresets
import com.forge.app.ui.common.ExerciseIcons
import com.forge.app.ui.theme.ForgeMotion

/**
 * The gym half of the generated path — the two steps where the [PlanLedger] under the question stops
 * being an empty shape and starts filling. Picking a preset deals the week; toggling one piece of
 * gear moves the meters while you watch. Then the week page, where that same mark stops being a
 * readout and becomes the way you read the week.
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
 * Last step of the generated path — the week, rebuilt 2026-08-23.
 *
 * What it replaced: the same bar mark the user had been reading for three screens, drawn again at
 * full size, and then every exercise of every day dumped underneath it in one column. A four-day
 * week ran to roughly twenty-five uniform rows and three viewports, which §3 bans outright for this
 * archetype (only the optional closing step is exempt from the long scroll), and the volume of each
 * day was stated three separate times — once by a bar, once by a day header, once by its rows.
 *
 * What it is now: **the mark IS the week, and it navigates.** One bar per training day carrying that
 * day's sets, accent on the day you are reading and muted on the rest, and under it that one day in
 * full — its movements, each with its equipment glyph and its sets by reps. Tapping a bar swaps the
 * detail. That is §4.5's "aggregate visuals answer a tap with detail", it gives the mark a job
 * instead of repeating one it already did (§4.3), and it puts the whole page — every day reachable,
 * the CTA and the re-roll — inside one viewport.
 *
 * Approving what you cannot see was the thing to get right, and the rail is what answers it: every
 * day of the week is on screen with its real volume from the moment the page opens, one tap from
 * being read in full. Nothing is hidden, only stacked.
 *
 * The per-day colour dot each row used to carry went with the rewrite: seven hues at 8dp is exactly
 * the "scattered tiny accent" §5 forbids, and day identity already rides the mono name.
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
    // Which day is open. Survives a re-roll on purpose — the split is unchanged, so the user stays
    // on the day they were reading and watches its movements change under them. Coerced rather than
    // reset, so it can never index off a shorter week.
    var picked by rememberSaveable { mutableIntStateOf(0) }
    val index = picked.coerceIn(0, (archetypes.size - 1).coerceAtLeast(0))
    val day = days.getOrNull(index)
    // A one-day week has nothing to move between, so it gets no tap affordance and no line telling
    // the user to use one. Its single bar IS the week, which is also why the day below drops its set
    // count: at one day that number and the week total are the same fact (§4.3).
    val many = archetypes.size > 1

    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        StepTitle("Here's your week")
        StepCaption(
            if (many) "Tap a day to read it. Change any of it in the editor."
            else "Built from your answers. Change any of it in the editor."
        )
        Spacer(Modifier.height(6.dp))
        // The anchor says days and sets because the page title already said "your week" (§4.3).
        PlanLedger(
            archetypes = archetypes,
            plannedSets = plannedSets,
            days = days,
            trackHeight = 104.dp,
            label = if (many) "${archetypes.size} days" else "1 day",
            selectedIndex = index,
            onSelect = if (many) ({ picked = it }) else null
        )
        Spacer(Modifier.height(8.dp))
        // Crossfades on both moves the list can make: switching day, and a re-roll dealing fresh
        // movements into the day already open.
        AnimatedContent(
            targetState = day,
            transitionSpec = { fadeIn(ForgeMotion.enterTween()) togetherWith fadeOut(ForgeMotion.exitTween()) },
            label = "week_day"
        ) { shown ->
            WeekDay(
                shown,
                fallbackName = archetypes.getOrNull(index)?.name.orEmpty(),
                showSets = many
            )
        }
    }
}

/**
 * The open day: mono anchor with its own reading as right meta, then one row per movement — the
 * equipment-class glyph (§8, same family the pickers lead with), the name, and the sets by reps at
 * the quiet mono rung so the movement names carry the row.
 */
@Composable
private fun WeekDay(day: GeneratedDay?, fallbackName: String, showSets: Boolean) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val exercises = day?.exercises.orEmpty()
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        StepSectionLabel(
            day?.name ?: fallbackName,
            meta = when {
                exercises.isEmpty() -> null
                showSets -> "${exercises.size} moves · ${exercises.sumOf { it.sets }} sets"
                else -> "${exercises.size} moves"
            }
        )
        if (exercises.isEmpty()) {
            // Effectively unreachable (the generator keeps a last-resort bodyweight fill), but a
            // blank day must still say what it means rather than mislabel itself.
            Text(
                "Nothing your gear covers yet",
                style = MaterialTheme.typography.bodyMedium,
                color = muted
            )
        }
        exercises.forEach { ex ->
            val def = ExerciseLibrary.byId(ex.libId)
            Row(
                Modifier.fillMaxWidth().padding(vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Icon(
                    ExerciseIcons.forEquipment(def?.equipment.orEmpty()),
                    contentDescription = null,
                    tint = muted,
                    modifier = Modifier.size(20.dp)
                )
                Text(
                    def?.name ?: ex.libId,
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.weight(1f)
                )
                Text(
                    "${ex.sets} × ${ex.reps}",
                    style = MaterialTheme.typography.labelMedium,
                    color = muted
                )
            }
        }
    }
}
