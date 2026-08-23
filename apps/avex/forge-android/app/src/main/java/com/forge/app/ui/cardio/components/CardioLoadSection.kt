package com.forge.app.ui.cardio.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.cardio.CardioWeekPoint
import com.forge.app.domain.cardio.WHO_WEEKLY_ACTIVITY_MIN
import com.forge.app.domain.cardio.cardioLoadDeltaPct
import com.forge.app.domain.cardio.cardioWeeksOnTarget
import com.forge.app.ui.common.EditorialHeader
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

/** How tall a load bar's track is. The month rail and the target line share it. */
private val LOAD_TRACK = 64.dp

/**
 * LOAD — weekly minutes across the last stretch of weeks, the reading cardio could not give before:
 * this week is a number, but whether the numbers are going up is the actual question. One bar per
 * week oldest→newest, the week still in progress drawn as a dashed slot so a Monday never reads as a
 * collapse, and the target (or the WHO 150-minute reference) drawn across them as a line — a line
 * as data, which is the only kind §1 allows.
 *
 * Empty weeks are present at zero on purpose: a load chart that drops the weeks nobody trained in
 * reads as an unbroken run.
 */
@Composable
internal fun CardioLoadSection(
    series: List<CardioWeekPoint>,
    /** The personal weekly minutes target; 0 falls back to the WHO reference, as the hero meter does. */
    weekTargetMin: Int,
    zone: ZoneId,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color,
    modifier: Modifier = Modifier
) {
    if (series.isEmpty()) return
    val target = if (weekTargetMin > 0) weekTargetMin else WHO_WEEKLY_ACTIVITY_MIN
    // The scale holds the target so the reference line is always on the chart, even in a quiet stretch.
    val peak = remember(series, target) {
        (series.maxOfOrNull { it.minutes } ?: 0).coerceAtLeast(target).coerceAtLeast(1)
    }
    val delta = remember(series) { cardioLoadDeltaPct(series) }
    val monthLetters = remember(series, zone) { monthRail(series, zone) }

    Column(modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            EditorialHeader(label = "Load", muted = muted, accent = accent, modifier = Modifier.weight(1f))
            // The deciding reading rides the header (§4.9) — how this week stands against the weeks
            // behind it, which is what the bars are being read FOR.
            if (delta != null) {
                Text(
                    if (delta >= 0) "↑ $delta% ON YOUR USUAL" else "↓ ${-delta}% ON YOUR USUAL",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted, letterSpacing = 0.5.sp
                )
            }
        }
        Spacer(Modifier.height(12.dp))

        val lastIndex = series.lastIndex
        Box(
            Modifier
                .fillMaxWidth()
                .semantics(mergeDescendants = true) {
                    contentDescription = loadReading(series, target)
                }
        ) {
            VerticalBarRow(
                count = series.size,
                trackHeight = LOAD_TRACK,
                modifier = Modifier.fillMaxWidth(),
                spacing = 4.dp,
                bar = { i ->
                    val week = series[i]
                    val frac = (week.minutes.toFloat() / peak).coerceIn(0f, 1f)
                    when {
                        // The week still running is a dashed slot at its height so far — honest about
                        // being unfinished rather than looking like a bad week.
                        i == lastIndex -> BarGeom(
                            height = (3 + 61 * frac).dp.coerceAtLeast(6.dp),
                            dashedOutline = accent
                        )
                        week.isEmpty -> BarGeom(height = 3.dp, fill = outline.copy(alpha = 0.35f))
                        else -> BarGeom(height = (3 + 61 * frac).dp, fill = accent)
                    }
                },
                bottom = { i ->
                    // A month letter only where the month turns over — 10 identical week labels would
                    // be debris, and the reader only needs to know how far back the chart reaches.
                    Text(
                        monthLetters[i],
                        style = MaterialTheme.typography.labelSmall,
                        color = if (i == lastIndex) onBg else muted,
                        fontWeight = if (i == lastIndex) FontWeight.Bold else FontWeight.Normal
                    )
                }
            )
            // The target rule, laid over the bars at its own height on the same scale.
            Canvas(
                Modifier
                    .fillMaxWidth()
                    .height(LOAD_TRACK)
                    // Sits below whatever the row reserves for its bottom labels.
                    .align(Alignment.TopCenter)
            ) {
                val y = size.height * (1f - (target.toFloat() / peak).coerceIn(0f, 1f))
                drawLine(
                    color = outline.copy(alpha = 0.35f),
                    start = Offset(0f, y),
                    end = Offset(size.width, y),
                    strokeWidth = 1.dp.toPx(),
                    pathEffect = PathEffect.dashPathEffect(floatArrayOf(6f, 6f), 0f)
                )
            }
        }
        Spacer(Modifier.height(10.dp))
        Text(
            loadCaption(series, target, weekTargetMin > 0).uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = muted, letterSpacing = 1.sp
        )
    }
}

/**
 * The bottom rail: a month initial on the first week of each month, a middle dot elsewhere. Reading
 * the FIRST label always, so a chart never opens on an unlabelled bar.
 */
private fun monthRail(series: List<CardioWeekPoint>, zone: ZoneId): List<String> {
    val fmt = DateTimeFormatter.ofPattern("MMM", Locale.getDefault())
    var lastMonth = -1
    return series.mapIndexed { i, week ->
        val date = Instant.ofEpochMilli(week.weekStartMs).atZone(zone).toLocalDate()
        val month = date.monthValue
        val turned = month != lastMonth
        lastMonth = month
        if (turned || i == 0) date.format(fmt).take(1).uppercase() else "·"
    }
}

/** "6 of 9 weeks cleared 150 min" — the one caption the section is allowed (§4.3). */
private fun loadCaption(series: List<CardioWeekPoint>, target: Int, personal: Boolean): String {
    val completed = series.size - 1
    if (completed <= 0) return "First week on the chart"
    val hit = cardioWeeksOnTarget(series, target)
    val reference = if (personal) "your $target min target" else "$target min"
    return "$hit of $completed weeks cleared $reference"
}

/** TalkBack's read of the chart: its values, not its shape (§14). */
private fun loadReading(series: List<CardioWeekPoint>, target: Int): String {
    val completed = series.size - 1
    val hit = cardioWeeksOnTarget(series, target)
    return "Weekly minutes over ${series.size} weeks, this week ${series.last().minutes} minutes, " +
        "$hit of $completed earlier weeks cleared $target"
}
