package com.forge.app.ui.gym.session

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.forge.app.ui.gym.stats.components.TargetBar
import com.forge.app.ui.gym.stats.components.rememberDrawProgress
import com.forge.app.ui.gym.stats.components.staggeredProgress
import com.forge.app.ui.gym.stats.state.MuscleSetCount

/**
 * "MUSCLES WORKED" — one labelled bar per muscle group the session hit, scaled to the busiest group
 * so the split reads at a glance. Reuses the [TargetBar] primitive + the stats reveal motion so it
 * matches the rest of the analytics surfaces. Caller wraps it in a StatCard.
 */
@Composable
internal fun MusclesWorkedCard(
    split: List<MuscleSetCount>,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val max = (split.maxOfOrNull { it.sets } ?: 1).coerceAtLeast(1)
    val overall = rememberDrawProgress(split.size)
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        split.forEachIndexed { i, m ->
            Column(verticalArrangement = Arrangement.spacedBy(5.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(m.muscle.displayName, style = MaterialTheme.typography.bodyMedium, color = onBg)
                    Text(
                        "${m.sets} ${if (m.sets == 1) "set" else "sets"}",
                        style = MaterialTheme.typography.labelMedium, color = muted
                    )
                }
                TargetBar(
                    fraction = m.sets.toFloat() / max,
                    targetFraction = null,
                    fillColor = accent,
                    trackColor = outline.copy(alpha = 0.18f),
                    tickColor = Color.Transparent,
                    modifier = Modifier.fillMaxWidth().height(6.dp),
                    progress = staggeredProgress(overall, i, split.size)
                )
            }
        }
    }
}
