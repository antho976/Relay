package com.forge.app.ui.onboarding

import com.forge.app.ui.common.ForgeGroupSection
import com.forge.app.ui.common.ForgeLabelTile
import com.forge.app.ui.common.ForgeTileGrid
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.Equipment
import com.forge.app.program.ProblemArea

/**
 * The sore / injured spots step, its own page since 2026-08-22 (it was four words and a chip cloud
 * at the bottom of a settings dump before, which is not where you put the question that decides
 * whether the app hands an injured lifter the movement that hurts).
 *
 * It sits AFTER the gym steps and BEFORE the week, so the week the user is shown is already the one
 * their flags produced — asking afterwards would have shaped a plan they had already approved.
 *
 * Each spot carries its own reading (§4.9): how many of the movements THIS gym supports load that
 * joint. That is the reason to flag one, and it is a stable property of the user's own equipment —
 * counting the current week instead was tried first and read worse, because a good roll could show
 * "Shoulders, 0 movements" to somebody whose shoulders are the reason they are on this page.
 *
 * Flagging is a preference, not a ban, and the caption says so rather than promising the movement is
 * gone. The week meter is not drawn here (2026-09-25): it held still while you flagged, so it
 * answered nothing on this page and pushed the tiles down.
 */

/** Head to toe, split where the body does. The order inside each half is anatomical, not by count. */
private val UPPER_SPOTS = listOf(ProblemArea.NECK, ProblemArea.SHOULDERS, ProblemArea.ELBOWS, ProblemArea.WRISTS)
private val LOWER_SPOTS = listOf(ProblemArea.LOWER_BACK, ProblemArea.HIPS, ProblemArea.KNEES, ProblemArea.ANKLES)

@Composable
internal fun StepSoreSpots(
    selected: Set<String>,
    equipment: Set<String>,
    frozenIds: Set<String>?,
    onToggle: (String) -> Unit
) {
    // How much of the pool this gym actually supports loads each joint — the same pool the generator
    // draws from, so the number is the real size of what flagging steers away from.
    val loadedBy = remember(equipment, frozenIds) {
        val available = equipment.mapNotNull { runCatching { Equipment.valueOf(it) }.getOrNull() }.toSet()
        val counts = mutableMapOf<ProblemArea, Int>()
        ExerciseLibrary.availablePool(available, frozenIds).forEach { def ->
            ExerciseLibrary.contraindicationsOf(def).forEach { area ->
                counts[area] = (counts[area] ?: 0) + 1
            }
        }
        counts
    }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        StepTitle("Any sore or injured spots?")
        StepCaption("Avex steers away from what loads them. A preference, not a ban.")
        Spacer(Modifier.height(6.dp))
        Column(verticalArrangement = Arrangement.spacedBy(24.dp)) {
            SpotGroup("Upper body", UPPER_SPOTS, selected, loadedBy, onToggle)
            SpotGroup("Lower body", LOWER_SPOTS, selected, loadedBy, onToggle)
        }
    }
}

@Composable
private fun SpotGroup(
    label: String,
    spots: List<ProblemArea>,
    selected: Set<String>,
    loadedBy: Map<ProblemArea, Int>,
    onToggle: (String) -> Unit
) {
    ForgeGroupSection(label, meta = "${spots.count { it.code in selected }} flagged") {
        ForgeTileGrid(spots, cols = 2) { area, corners, modifier ->
            val loaded = loadedBy[area] ?: 0
            // The count is a reading, never a state word, so a flagged tile says how much work is
            // being steered rather than announcing that it is on (the accent wash does that). Zero is
            // drawn honestly: a gym that never loads your ankles is worth knowing.
            ForgeLabelTile(
                label = area.displayName,
                meta = if (loaded == 1) "1 movement" else "$loaded movements",
                selected = area.code in selected,
                corners = corners,
                onClick = { onToggle(area.code) },
                modifier = modifier
            )
        }
    }
}
