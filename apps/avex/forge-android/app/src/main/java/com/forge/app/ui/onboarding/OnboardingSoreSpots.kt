package com.forge.app.ui.onboarding

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.Equipment
import com.forge.app.program.ProblemArea
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.ForgeMotion

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
 * gone. The week under it holds steady while you flag, which is true: steering changes WHICH
 * movements get picked, not how much work there is.
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
        SpotGroup("Upper body", UPPER_SPOTS, selected, loadedBy, onToggle)
        Spacer(Modifier.height(8.dp))
        SpotGroup("Lower body", LOWER_SPOTS, selected, loadedBy, onToggle)
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
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        StepSectionLabel(label, meta = "${spots.count { it.code in selected }} flagged")
        spots.chunked(2).forEach { pair ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                pair.forEach { area ->
                    SpotTile(
                        label = area.displayName,
                        loaded = loadedBy[area] ?: 0,
                        selected = area.code in selected,
                        onClick = { onToggle(area.code) },
                        modifier = Modifier.weight(1f)
                    )
                }
                if (pair.size == 1) Spacer(Modifier.weight(1f))
            }
        }
    }
}

/**
 * One spot: the joint, and the count of movements in the current week that load it. The count is the
 * §2① "qualifies a row that already exists" case — a reading, never a state word, so a flagged tile
 * says how much work is being steered rather than announcing that it is on (the accent wash does
 * that). Zero is drawn honestly: a week that never loads your ankles is worth knowing.
 */
@Composable
private fun SpotTile(
    label: String,
    loaded: Int,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val border by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primary
        else MaterialTheme.colorScheme.outline.copy(alpha = 0.35f),
        ForgeMotion.standardTween(ForgeMotion.DurationFast),
        label = "spot_border"
    )
    val fill by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primary.copy(alpha = 0.15f)
        else androidx.compose.ui.graphics.Color.Transparent,
        ForgeMotion.standardTween(ForgeMotion.DurationFast),
        label = "spot_fill"
    )
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(12.dp))
            .border(1.dp, border, RoundedCornerShape(12.dp))
            .background(fill)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected }
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        Text(
            label,
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.onBackground
        )
        Text(
            if (loaded == 1) "1 MOVEMENT" else "$loaded MOVEMENTS",
            style = MaterialTheme.typography.labelSmall,
            color = if (selected) MaterialTheme.colorScheme.primary
            else MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}
