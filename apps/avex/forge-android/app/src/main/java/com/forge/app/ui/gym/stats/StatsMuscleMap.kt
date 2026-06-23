package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.gym.stats.components.BodyHeatmap
import com.forge.app.ui.gym.stats.components.statsEntrance
import com.forge.app.ui.gym.stats.state.MuscleSetCount

/**
 * Tier 3 — muscle distribution as an anatomical heatmap (front + back), each region tinted by this
 * week's set count, plus a "most neglected" read against the plan's targets. The headline "where am I
 * under-training?" visual.
 */
internal fun LazyListScope.muscleMapSection(
    weekly: List<MuscleSetCount>,
    planned: Map<MuscleGroup, Int>,
    c: StatsColors
) {
    item("muscle-map") {
        val setsBy = weekly.associate { it.muscle to it.sets }
        Column(Modifier.fillMaxWidth().statsEntrance(0).padding(horizontal = STATS_GUTTER)) {
            BodyHeatmap(
                setsByMuscle = setsBy,
                accent = c.accent,
                faint = c.outline.copy(alpha = 0.34f),
                silhouette = c.outline.copy(alpha = 0.26f),
                labelColor = c.muted,
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(Modifier.height(10.dp))

            val under = planned.entries
                .filter { it.value > 0 }
                .map { (m, target) -> m to ((setsBy[m] ?: 0).toDouble() / target) }
                .filter { it.second < 1.0 }
                .sortedBy { it.second }
                .take(3)
                .map { it.first.displayName }

            Text(
                if (under.isEmpty()) "You're hitting target volume across every planned muscle this week."
                else "Most neglected vs plan: ${under.joinToString(", ")}.",
                style = MaterialTheme.typography.bodySmall,
                color = c.muted
            )
        }
    }
}
