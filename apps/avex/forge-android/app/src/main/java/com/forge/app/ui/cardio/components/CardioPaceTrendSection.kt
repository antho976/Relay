package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioPaceSeries
import com.forge.app.domain.cardio.formatPaceSec
import com.forge.app.domain.cardio.paceSecPerUnit
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.ui.cardio.LocalCardioTypes
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeSlidingSegments
import com.forge.app.ui.cardio.CardioChartPanel
import com.forge.app.ui.cardio.CardioFilledChip
import com.forge.app.ui.gym.stats.components.LineChart
import com.forge.app.ui.gym.stats.components.olsTrend
import com.forge.app.ui.common.rememberDrawProgressState
import com.forge.app.ui.theme.ForgeMotion
import kotlin.math.roundToInt

/**
 * PACE TREND (GYMAP-35) — a per-activity pace-over-time line for the week overlay's current page. A
 * type-pill selector chooses which activity to read (only types with two or more paced sessions
 * qualify); the chart plots that type's pace per session oldest→newest with an OLS trend under it.
 * Pace is "lower = faster", so a line trending DOWN is improvement — the caption says it in words.
 * Cross-week data, shown on the current page alone so it never restates another page's answer (§4.3).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun CardioPaceTrendSection(
    series: List<CardioPaceSeries>,
    useMiles: Boolean,
    muted: Color,
    accent: Color
) {
    if (series.isEmpty()) return
    val customs = LocalCardioTypes.current
    val secondary = MaterialTheme.colorScheme.secondary
    // Reset the pick when the set of qualifying types changes (a new activity earning its 2nd session).
    var selected by remember(series.map { it.typeCode }) { mutableIntStateOf(0) }
    val sel = series.getOrElse(selected) { series.first() }
    val unit = distanceUnitLabel(useMiles)
    // Pace per point in the viewer's unit — the single rounding path, so it matches each session's read.
    val paces = remember(sel, useMiles) {
        sel.points.mapNotNull { paceSecPerUnit(it.durationMin, it.distanceKm, useMiles) }
    }
    // Handed to the chart as a lambda so only its draw reads the reveal, not this section.
    val progress = rememberDrawProgressState(sel.typeCode, ForgeMotion.drawTween())
    val paceValues = remember(paces) { paces.map { it.toDouble() } }

    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        EditorialHeader(label = "Pace trend", muted = muted, accent = accent)
        Spacer(Modifier.height(10.dp))
        if (series.size > 1) {
            // The lens over which activity's pace to read: a few activities are one segmented
            // control; a longer set wraps as filled chips rather than squeezing the cells.
            val names = series.map { CardioActivity.resolve(it.typeCode, customs).displayName }
            if (names.size <= 3) {
                ForgeSlidingSegments(
                    options = names,
                    selectedIndex = selected,
                    onSelect = { selected = it },
                    modifier = Modifier.fillMaxWidth()
                )
            } else {
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    names.forEachIndexed { i, name ->
                        CardioFilledChip(name, selected = i == selected, onClick = { selected = i })
                    }
                }
            }
            Spacer(Modifier.height(12.dp))
        }
        if (paces.size >= 2) {
            CardioChartPanel {
                LineChart(
                    values = paceValues,
                    lineColor = accent,
                    trendColor = secondary,
                    modifier = Modifier.fillMaxWidth().height(100.dp),
                    progress = { progress.value }
                )
                Spacer(Modifier.height(10.dp))
                Text(
                    paceTrendCaption(paces, unit),
                    style = MaterialTheme.typography.labelSmall,
                    color = muted, fontSize = 9.sp
                )
            }
        }
    }
}

/**
 * A dry read of the pace series: the latest pace plus the net direction over the whole history (a
 * lower pace is faster), or just the latest when the fit is flat. §11 — grounded in the numbers.
 */
private fun paceTrendCaption(paces: List<Int>, unit: String): String {
    val latest = "${formatPaceSec(paces.last())}/$unit latest"
    val trend = olsTrend(paces.map { it.toDouble() }) ?: return latest
    val deltaSec = (trend.first * (paces.size - 1)).roundToInt()
    return when {
        deltaSec <= -1 -> "$latest · ${-deltaSec}s faster"
        deltaSec >= 1 -> "$latest · ${deltaSec}s slower"
        else -> "$latest · steady"
    }
}
