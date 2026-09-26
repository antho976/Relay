package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.gym.stats.components.BodyHeatmap
import com.forge.app.ui.gym.stats.state.PrRecord
import com.forge.app.ui.gym.stats.state.StatsUiState
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt

/**
 * The Stats hero + the always-on Records list. The hero mirrors the session-detail header — the
 * page's "face": THIS WEEK's three serif figures (volume · sessions · sets, with vs-last deltas) on
 * the left, the compact weekly muscle map on the right, and the one-line readiness status underneath
 * (2026-07-01 fusion — the old top-lift hero dissolved into the Strength lens, where it's simply the
 * first row).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun ColumnScope.StatsHeroContent(state: StatsUiState, weightUnit: WeightUnit, c: StatsColors) {
    val cmp = state.weekComparison
    val cur = cmp?.current
    val prev = cmp?.previous
    // A delta needs a baseline. Against an empty last week every figure read "▲ n vs last", three
    // identical accent lines that said nothing, so a figure only carries its arrow once last week
    // had something to compare against.
    val volumeDelta = if (prev != null && prev.volumeLb > 0.0)
        toDisplayWeight(cmp.volumeDelta, weightUnit).roundToInt() else null
    val sessionsDelta = if (prev != null && prev.sessions > 0) cmp.sessionsDelta else null
    val setsDelta = if (prev != null && prev.sets > 0) cur!!.sets - prev.sets else null
    val anyDelta = listOfNotNull(volumeDelta, sessionsDelta, setsDelta).any { it != 0 }

    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
        Column(Modifier.weight(1f)) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                EditorialHeader(label = "This week", muted = c.muted, accent = c.accent, modifier = Modifier.weight(1f))
                // What the arrows are measured against, said once in the header rather than
                // repeated under every figure (Home's THIS WEEK carries its meta the same way).
                if (anyDelta) {
                    Text("VS LAST", style = MaterialTheme.typography.labelSmall, color = c.muted.copy(alpha = 0.65f))
                }
            }
            Spacer(Modifier.height(14.dp))
            // The shared figure, so these read exactly like History's and Home's. Honest zeros on a
            // first run or a quiet week (§12), never a dash. Wraps rather than clips at large font.
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(20.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                EditorialFigure(
                    value = formatVolumeCompact(cur?.volumeLb ?: 0.0, weightUnit),
                    label = "Volume",
                    onBg = c.onBg, muted = c.muted, accent = c.accent,
                    delta = volumeDelta
                )
                EditorialFigure(
                    value = "${cur?.sessions ?: 0}",
                    label = if (cur?.sessions == 1) "Session" else "Sessions",
                    onBg = c.onBg, muted = c.muted, accent = c.accent,
                    delta = sessionsDelta
                )
                EditorialFigure(
                    value = "${cur?.sets ?: 0}",
                    label = if (cur?.sets == 1) "Set" else "Sets",
                    onBg = c.onBg, muted = c.muted, accent = c.accent,
                    delta = setsDelta
                )
            }
        }
        // The weekly muscle map as the header's face — same spot as the session screen's body figure.
        // Always drawn: at zero it's the faint silhouette (the section's own visual at zero), so the hero
        // carries a mark before the first log instead of a gap (§12).
        Spacer(Modifier.width(16.dp))
        BodyHeatmap(
            setsByMuscle = state.weeklySetsByMuscle.associate { it.muscle to it.sets },
            accent = c.accent,
            faint = c.outline.copy(alpha = 0.35f),
            silhouette = c.outline.copy(alpha = 0.25f),
            labelColor = c.muted,
            figureHeight = 104.dp,
            showLegend = false,
            showTitles = false,
            modifier = Modifier.width(108.dp)
        )
    }
    if (state.readinessPulse != null && state.readinessThreshold != null) {
        Spacer(Modifier.height(16.dp))
        ReadinessLine(state.readinessPulse, state.readinessThreshold, c)
    }
}

/** Records — the all-time heaviest set per lift, biggest first. Rows tap through to the lift's trend. */
@Composable
internal fun ColumnScope.RecordsContent(
    records: List<PrRecord>,
    weightUnit: WeightUnit,
    c: StatsColors,
    onOpenLift: (String) -> Unit = {}
) {
    val fmt = remember { SimpleDateFormat("MMM d, yyyy", Locale.getDefault()) }
    val shown = records.take(6)
    shown.forEachIndexed { i, r ->
        Row(
            Modifier.fillMaxWidth()
                .clickableLabeled("Show estimated 1RM trend for ${r.exerciseName}") { onOpenLift(r.exerciseId) }
                .padding(vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(Modifier.weight(1f)) {
                Text(r.exerciseName, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
                Spacer(Modifier.height(2.dp))
                Text(fmt.format(Date(r.sessionDate)).uppercase(), style = MaterialTheme.typography.labelSmall, color = c.muted)
            }
            Spacer(Modifier.width(12.dp))
            // onBg, not accent: a record is content, and accent body text fails contrast under most
            // accents (§14). formatWeight keeps the real load, so a 7.5 kg set no longer reads "7 kg".
            Text(
                "${formatWeight(r.maxWeightLb, weightUnit)} × ${r.bestReps}",
                style = MaterialTheme.typography.titleSmall,
                color = c.onBg
            )
        }
        // Table rule between record rows — a data line on the §5 hairline rung.
        if (i < shown.lastIndex) androidx.compose.material3.HorizontalDivider(color = c.outline.copy(alpha = 0.25f))
    }
}
