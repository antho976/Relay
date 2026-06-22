package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.ui.gym.stats.components.CalendarHeatmap
import com.forge.app.ui.gym.stats.components.statsEntrance
import com.forge.app.ui.gym.stats.state.DayLoad
import java.time.LocalDate

private const val HEATMAP_WEEKS = 26

/**
 * Tier 7 — adherence calendar heatmap. "Did I actually show up?" answered instantly by the density of
 * lit days over the last [HEATMAP_WEEKS] weeks.
 */
internal fun LazyListScope.adherenceSection(dailyActivity: List<DayLoad>, c: StatsColors) {
    if (dailyActivity.isEmpty()) return
    item("adherence") {
        val byDay = remember(dailyActivity) { dailyActivity.associate { it.epochDay to it.sets } }
        // Count only the days the heatmap actually shows (last HEATMAP_WEEKS weeks), so the label can't
        // claim "312 training days · last 26 weeks" when most of those days are off-screen history.
        val windowDays = remember(dailyActivity) {
            val cutoff = LocalDate.now().toEpochDay() - HEATMAP_WEEKS * 7L
            dailyActivity.count { it.epochDay >= cutoff }
        }
        Column(Modifier.fillMaxWidth().statsEntrance(0).padding(horizontal = STATS_GUTTER)) {
            CalendarHeatmap(
                activityByDay = byDay,
                weeks = HEATMAP_WEEKS,
                faint = c.outline.copy(alpha = 0.18f),
                accent = c.accent,
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(Modifier.height(8.dp))
            Text(
                "$windowDays training days in the last $HEATMAP_WEEKS weeks · scroll the grid →",
                style = MaterialTheme.typography.labelSmall,
                color = c.muted
            )
        }
    }
}
