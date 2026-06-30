package com.forge.app.ui.gym.stats

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.gym.stats.components.LineChart
import com.forge.app.ui.gym.stats.components.Sparkline
import com.forge.app.ui.gym.stats.state.E1rmLift
import kotlin.math.roundToInt

/**
 * Strength tier (#1 spine chart): estimated 1RM over time, one row per lift. Each row carries a
 * sparkline of the lift's e1RM history; tapping expands a full [LineChart] with the OLS trend drawn
 * through it — "the line climbing is both the information and the payoff."
 *
 * X is the session ordinal (oldest → newest), not a calendar axis yet — `E1rmLift.history` is a bare
 * per-session series. A dated axis is a later tweak (needs the repo to carry timestamps per point).
 */
@Composable
internal fun ColumnScope.E1rmLedgerContent(lifts: List<E1rmLift>, useKg: Boolean, c: StatsColors) {
    lifts.forEachIndexed { index, lift ->
        E1rmLiftRow(lift, useKg, c, showDivider = index < lifts.lastIndex)
    }
}

@Composable
private fun E1rmLiftRow(lift: E1rmLift, useKg: Boolean, c: StatsColors, showDivider: Boolean) {
    var expanded by rememberSaveable(lift.exerciseId) { mutableStateOf(false) }
    val display = remember(lift.history, useKg) { lift.history.map { toDisplayWeight(it, useKg) } }
    val current = toDisplayWeight(lift.currentE1rm, useKg).roundToInt()
    val unit = unitLabel(useKg)
    val lo = remember(display) { display.minOrNull() ?: 0.0 }
    val hi = remember(display) { display.maxOrNull() ?: 1.0 }

    Column(
        Modifier
            .fillMaxWidth()
            .clickableLabeled(
                if (expanded) "Hide ${lift.exerciseName} trend"
                else "Show estimated 1RM trend for ${lift.exerciseName}"
            ) { expanded = !expanded }
            .padding(vertical = 10.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(lift.exerciseName, style = MaterialTheme.typography.bodyLarge, color = c.onBg)
                Spacer(Modifier.height(2.dp))
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        "$current $unit e1RM",
                        style = MaterialTheme.typography.labelMedium,
                        color = c.accent
                    )
                    lift.monthlyPct?.let { pct ->
                        Text(
                            "%+.1f%%/mo".format(pct),
                            style = MaterialTheme.typography.labelSmall,
                            color = if (pct >= 0) c.accent else c.muted
                        )
                    }
                    if (lift.stalling) {
                        Text(
                            "stalling",
                            style = MaterialTheme.typography.labelSmall,
                            color = c.muted,
                            fontStyle = FontStyle.Italic
                        )
                    }
                }
            }
            if (display.size >= 2) {
                Spacer(Modifier.width(12.dp))
                Sparkline(
                    values = display,
                    lineColor = c.accent,
                    minValue = lo,
                    maxValue = hi,
                    modifier = Modifier.width(72.dp).height(34.dp)
                )
            }
        }

        AnimatedVisibility(
            visible = expanded,
            enter = expandVertically() + fadeIn(),
            exit = shrinkVertically() + fadeOut()
        ) {
            Column(Modifier.fillMaxWidth().padding(top = 12.dp)) {
                if (display.size >= 2) {
                    LineChart(
                        values = display,
                        lineColor = c.accent,
                        trendColor = c.muted,
                        minValue = lo,
                        maxValue = hi,
                        modifier = Modifier.fillMaxWidth().height(STATS_CHART_H)
                    )
                    Spacer(Modifier.height(6.dp))
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Text("${lo.roundToInt()} $unit", style = MaterialTheme.typography.labelSmall, color = c.muted)
                        Text("dashed line = trend", style = MaterialTheme.typography.labelSmall, color = c.muted)
                        Text("${hi.roundToInt()} $unit", style = MaterialTheme.typography.labelSmall, color = c.muted)
                    }
                } else {
                    Text(
                        "A couple more logged sessions and the trend line shows up here.",
                        style = MaterialTheme.typography.bodySmall,
                        color = c.muted
                    )
                }
            }
        }

        if (showDivider) {
            Spacer(Modifier.height(10.dp))
            HorizontalDivider(color = c.outline.copy(alpha = 0.25f))
        }
    }
}
