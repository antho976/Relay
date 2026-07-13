package com.forge.app.ui.profile

import androidx.compose.foundation.layout.Column
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.LocalDate
import kotlin.math.abs
import kotlin.math.roundToInt

/** The comparison window for the little "vs a month ago" delta beside the current weight. */
private const val DELTA_WINDOW_MS = 30L * 86_400_000L

/**
 * BODYWEIGHT — your weight belongs on your profile, not buried in Stats (moved 2026-07-01). The
 * current weight as an open serif figure with a ~30-day delta beside it, the recent trend as a
 * quiet sparkline (a 7-day average line over the raw weigh-ins, plus the dashed goal line when a
 * bodyweight goal is set), and the quick-log behind the header's "+ log" action (the only manual
 * weigh-in entry point after onboarding).
 */
@Composable
internal fun BodySection(
    entries: List<BodyweightEntry>,
    goalLb: Double? = null,
    onLog: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    val unit = unitLabel(weightUnit)
    SectionHeader("BODYWEIGHT", muted, action = "+ log", onAction = onLog)
    if (entries.isEmpty()) {
        // Just a quiet hint under the header — the "+ log" action IS the entry point. A bold CTA row
        // here read as out of place between the populated sections.
        InlineEmptyHint("Log a weigh-in and your weight trend charts here.", muted)
        return
    }
    val display = remember(entries, weightUnit) { entries.map { toDisplayWeight(it.weightLb, weightUnit) } }
    // The smoothed trend the arrow + chart line read from — raw weigh-ins swing day to day (water).
    val ma = remember(entries, display) { sevenDayMovingAverage(entries, display) }
    // Trend vs the smoothed weight ~30 days ago (falls back to the previous point). Off the moving
    // average, not the raw series, so a single noisy final weigh-in can't flip the arrow.
    val delta = remember(entries, ma) {
        if (entries.size < 2) null else {
            val cutoff = entries.last().recordedAt - DELTA_WINDOW_MS
            val refIdx = entries.indexOfFirst { it.recordedAt >= cutoff }.coerceAtMost(entries.lastIndex - 1)
            ma.last() - ma[refIdx.coerceAtLeast(0)]
        }
    }
    Row(verticalAlignment = Alignment.Top) {
        Text(
            "${display.last().roundToInt()}",
            style = MaterialTheme.typography.headlineLarge,
            color = onBg
        )
        delta?.let {
            if (abs(it) >= 0.05) {
                Spacer(Modifier.width(8.dp))
                Text(
                    "${if (it > 0) "↑" else "↓"} %.1f $unit · 30 days".format(abs(it)),
                    style = MaterialTheme.typography.labelSmall,
                    color = muted, fontSize = 9.sp,
                    modifier = Modifier.padding(top = 6.dp)
                )
            }
        }
    }
    Spacer(Modifier.height(2.dp))
    Text("${unit.uppercase()} NOW", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
    if (display.size >= 2) {
        val goalDisplay = goalLb?.let { toDisplayWeight(it, weightUnit) }
        Spacer(Modifier.height(14.dp))
        // The 7-day average is the bold trend line; the raw weigh-ins are the muted scatter around it;
        // the goal (when set) is the dashed target the range always keeps on-canvas.
        ProfileSparkline(
            values = ma,
            color = accent,
            modifier = Modifier.fillMaxWidth().height(56.dp),
            points = display,
            pointColor = muted,
            reference = goalDisplay
        )
        Spacer(Modifier.height(8.dp))
        ChartCaption(
            accent,
            if (goalDisplay != null) "LAST ${entries.size} WEIGH-INS · GOAL ${goalDisplay.roundToInt()} ${unit.uppercase()}"
            else "LAST ${entries.size} WEIGH-INS",
            muted
        )
    }
}

/**
 * Trailing 7-calendar-day moving average aligned index-for-index with [display] (and [entries]):
 * each point averages the display weights of every weigh-in whose day is within the 6 days before
 * it (inclusive). One row per calendar day (unique date_key), so a window holds at most 7 values;
 * a weekly logger's average simply tracks the raw line.
 */
private fun sevenDayMovingAverage(entries: List<BodyweightEntry>, display: List<Double>): List<Double> {
    if (entries.isEmpty()) return emptyList()
    val days = entries.map { LocalDate.parse(it.dateKey).toEpochDay() }
    return display.indices.map { i ->
        val floor = days[i] - 6
        var sum = 0.0
        var n = 0
        // entries are chronological, so walk back from i while still inside the trailing window.
        var j = i
        while (j >= 0 && days[j] >= floor) { sum += display[j]; n++; j-- }
        sum / n
    }
}
