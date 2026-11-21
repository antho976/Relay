package com.forge.app.ui.onboarding

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.program.DayArchetype
import com.forge.app.program.Equipment
import com.forge.app.program.EquipmentPreset
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.GeneratedDay
import com.forge.app.program.equipmentGroups
import com.forge.app.program.equipmentPresets
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.OnboardingType
import com.forge.app.ui.theme.OpenIcons

/**
 * The gym half of the generated path: pick a setup, fine-tune the gear, flag sore spots, then the
 * week page. Redrawn 2026-10-21: the setups are a quiet two-column grid whose piece count is the
 * reading, the gear the same tiles three across.
 */

/** Rows of equal-height tiles, [cols] across. A short last row keeps its tiles at full-row width. */
@Composable
internal fun <T> TileGrid(
    items: List<T>,
    cols: Int,
    gap: Dp = 10.dp,
    cell: @Composable (item: T, modifier: Modifier) -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(gap)) {
        items.chunked(cols).forEach { row ->
            Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(gap)) {
                row.forEach { cell(it, Modifier.weight(1f).fillMaxHeight()) }
                repeat(cols - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

/** The house glyph for a setup tile, from the redesign's own icon set. */
private fun presetIcon(id: String): ImageVector = when (id) {
    "everything" -> OpenIcons.Store
    "basic-gym", "dumbbells" -> OpenIcons.Dumbbell
    "home-big", "home-small", "developer" -> OpenIcons.Home
    else -> OpenIcons.User
}

/** GYMAP-20 step 1 of 2: pick the closest gym preset. */
@Composable
internal fun StepGymPresets(
    selected: Set<String>,
    frozenIds: Set<String>?,
    onSelectPreset: (EquipmentPreset) -> Unit
) {
    Column {
        StepHeader("What's in your gym?", "Pick the closest setup. You fine-tune every piece next.")
        Spacer(Modifier.height(28.dp))
        // Two across until a long name ("Home gym · small") would break mid-word, then one.
        val cols = if (LocalDensity.current.fontScale > 1.3f) 1 else 2
        TileGrid(equipmentPresets, cols) { preset, modifier ->
            val picked = selected == preset.equipment && frozenIds == preset.frozenIds
            OnbTile(selected = picked, onClick = { onSelectPreset(preset) }, modifier = modifier) {
                Icon(
                    presetIcon(preset.id), contentDescription = null,
                    tint = if (picked) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(22.dp)
                )
                Spacer(Modifier.height(10.dp))
                Text(preset.label, style = OnboardingType.gridTitle, color = MaterialTheme.colorScheme.onBackground)
                Spacer(Modifier.height(10.dp))
                TileMeta(presetMeta(preset), picked)
            }
        }
    }
}

/** GYMAP-20 step 2 of 2: every piece of gear, grouped, toggleable — the same tiles, three across. */
@Composable
internal fun StepFineTune(selected: Set<String>, onToggle: (String) -> Unit) {
    Column {
        StepHeader("Fine-tune your gear", "Toggle anything the preset got wrong. Plans use only what's on.")
        Spacer(Modifier.height(28.dp))
        // Three across until the names would break mid-word ("Dumbbell / s"), then two.
        val cols = if (LocalDensity.current.fontScale > 1.3f) 2 else 3
        Column(verticalArrangement = Arrangement.spacedBy(32.dp)) {
            equipmentGroups.forEach { (group, items) ->
                Column {
                    StepSectionLabel(group, meta = "${items.count { it.name in selected }} on")
                    Spacer(Modifier.height(12.dp))
                    TileGrid(items, cols, gap = 8.dp) { e, modifier ->
                        val on = e.name in selected
                        OnbTile(
                            selected = on,
                            onClick = { onToggle(e.name) },
                            modifier = modifier,
                            multi = true,
                            contentPadding = PaddingValues(12.dp)
                        ) {
                            Icon(
                                OnboardingIcons.forEquipment(e), contentDescription = null,
                                tint = if (on) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.size(24.dp)
                            )
                            Spacer(Modifier.height(8.dp))
                            Text(e.display, style = OnboardingType.chip, color = MaterialTheme.colorScheme.onBackground)
                        }
                    }
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
 * Last plan step of the generated path: the week, drawn like the Program page. One bar per training
 * day carrying that day's sets, red on the day you are reading and grey on the rest; tapping a bar
 * opens that day underneath, its movements with their sets by reps. Every day is on screen with its
 * real volume from the moment the page opens, one tap from being read in full.
 *
 * The exact week shown is the week that gets saved (Re-roll sits beside the CTA in the footer; edits
 * live in the Program Editor).
 */
@Composable
internal fun StepWeek(
    archetypes: List<DayArchetype>,
    plannedSets: List<Int>,
    /** Null while the week is still being built; the bars stand on the planned volume meanwhile. */
    days: List<GeneratedDay>?
) {
    // Which day is open. Survives a re-roll on purpose — the split is unchanged, so the user stays
    // on the day they were reading and watches its movements change under them. Coerced rather than
    // reset, so it can never index off a shorter week.
    var picked by rememberSaveable { mutableIntStateOf(0) }
    val index = picked.coerceIn(0, (archetypes.size - 1).coerceAtLeast(0))
    val day = days?.getOrNull(index)
    // A one-day week has nothing to move between, so it gets no tap affordance and no line telling
    // the user to use one.
    val many = archetypes.size > 1
    val sets = weekSets(archetypes, plannedSets, days)

    Column(Modifier.fillMaxWidth()) {
        StepHeader(
            "Here's your week",
            if (many) "Tap a day to read it. Change any of it in the editor."
            else "Built from your answers. Change any of it in the editor."
        )
        Spacer(Modifier.height(28.dp))
        StepSectionLabel(if (many) "${archetypes.size} days" else "1 day", meta = "${sets.sum()} sets")
        Spacer(Modifier.height(14.dp))
        WeekBars(
            names = archetypes.map { it.name },
            sets = sets,
            trackWidth = 40.dp,
            trackHeight = 100.dp,
            labelGap = 8.dp,
            selectedIndex = index,
            onSelect = if (many) ({ picked = it }) else null
        )
        Spacer(Modifier.height(28.dp))
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
                showSets = many,
                building = days == null
            )
        }
    }
}

/** The open day: mono anchor with its own reading, then one row per movement and its sets by reps. */
@Composable
private fun WeekDay(day: GeneratedDay?, fallbackName: String, showSets: Boolean, building: Boolean = false) {
    val exercises = day?.exercises.orEmpty()
    Column {
        StepSectionLabel(
            day?.name ?: fallbackName,
            meta = when {
                exercises.isEmpty() -> null
                showSets -> "${exercises.size} moves · ${exercises.sumOf { it.sets }} sets"
                else -> "${exercises.size} moves"
            }
        )
        Spacer(Modifier.height(14.dp))
        // While the week is still being built the day shows its name alone: "nothing your gear
        // covers" would be a false statement about a week that doesn't exist yet.
        if (exercises.isEmpty() && !building) {
            // Effectively unreachable (the generator keeps a last-resort bodyweight fill), but a
            // blank day must still say what it means rather than mislabel itself.
            Text("Nothing your gear covers yet", style = OnboardingType.movement, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
            exercises.forEach { ex ->
                val def = ExerciseLibrary.byId(ex.libId)
                Row(
                    Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        def?.name ?: ex.libId,
                        style = OnboardingType.movement,
                        color = MaterialTheme.colorScheme.onBackground,
                        modifier = Modifier.weight(1f)
                    )
                    Text(
                        "${ex.sets} × ${rangeCopy(ex.reps)}",
                        style = OnboardingType.setsByReps,
                        color = MaterialTheme.colorScheme.onBackground
                    )
                }
            }
        }
    }
}
