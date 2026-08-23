package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.domain.cardio.CardioWeekPoint
import com.forge.app.ui.common.clickableLabeled
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

/** How tall a week bar's track is. The target rule is drawn on the same scale. */
private val WEEK_TRACK = 132.dp

/**
 * One bar per week, taller the more you did — the whole point of the weeks page. A bar is a tap
 * target that opens that week; the week still in progress is a dashed slot so a Monday never reads
 * as a collapse; a week nobody trained keeps a ghost stub rather than vanishing (§12), because a
 * chart that drops untrained weeks reads as an unbroken run.
 *
 * The target (or the WHO 150-minute reference) is drawn across the bars as a dashed rule — a line as
 * data, which is the only kind §1 allows.
 */
@Composable
internal fun CardioWeekBars(
    /** The visible window, oldest→newest. The screen pages this with its arrows. */
    weeks: List<CardioWeekPoint>,
    /** Minutes the bars are measured against — a personal target, else the WHO reference. */
    targetMin: Int,
    /** Monday 00:00 of the week in progress — drawn as a dashed slot when it is on screen. */
    currentWeekStartMs: Long,
    zone: ZoneId,
    onOpenWeek: (Long) -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color,
    modifier: Modifier = Modifier
) {
    if (weeks.isEmpty()) return
    // The scale holds the target, so the reference rule is always on the chart even in a quiet
    // stretch, and a tall week never pushes it off the bottom.
    val peak = remember(weeks, targetMin) {
        (weeks.maxOfOrNull { it.minutes } ?: 0).coerceAtLeast(targetMin).coerceAtLeast(1)
    }
    // The bar's own label is the day of the month — eight "18 Aug"s do not fit the gutter, and
    // the range in the header above already says which months are on screen.
    val labels = remember(weeks, zone) {
        weeks.map { Instant.ofEpochMilli(it.weekStartMs).atZone(zone).toLocalDate().dayOfMonth.toString() }
    }
    // The reading TalkBack gets, and what a bar means — the full date, never the bare day (§11).
    val readings = remember(weeks, zone) {
        val fmt = DateTimeFormatter.ofPattern("d MMM", Locale.getDefault())
        weeks.map { week ->
            val day = Instant.ofEpochMilli(week.weekStartMs).atZone(zone).toLocalDate().format(fmt)
            if (week.isEmpty) "Week of $day, nothing logged"
            else "Week of $day, ${week.minutes} minutes, ${week.sessions} sessions"
        }
    }

    // Where the target rule crosses each track, as a share of its height.
    val targetFraction = (targetMin.toFloat() / peak).coerceIn(0f, 1f)

    Row(
        modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalAlignment = Alignment.Bottom
    ) {
        weeks.forEachIndexed { i, week ->
            WeekBarColumn(
                week = week,
                label = labels[i],
                reading = readings[i],
                fraction = (week.minutes.toFloat() / peak).coerceIn(0f, 1f),
                targetFraction = targetFraction,
                isCurrent = week.weekStartMs == currentWeekStartMs,
                onClick = { onOpenWeek(week.weekStartMs) },
                onBg = onBg, muted = muted, outline = outline, accent = accent,
                modifier = Modifier.weight(1f)
            )
        }
    }
}

/**
 * One week's column: its minutes above, the bar, its date below. The WHOLE column is the tap target
 * (§2③) so the reach is the column's full height, not the bar's drawn height — a quiet week would
 * otherwise be almost impossible to hit.
 */
@Composable
private fun WeekBarColumn(
    week: CardioWeekPoint,
    label: String,
    reading: String,
    fraction: Float,
    /** Where the shared target rule crosses this track, as a share of its height. */
    targetFraction: Float,
    isCurrent: Boolean,
    onClick: () -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color,
    modifier: Modifier = Modifier
) {
    val ruleColor = outline.copy(alpha = 0.35f)
    Column(
        modifier
            .clickableLabeled(reading, onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = reading },
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        // The value row sizes to its text (§14) — every column carries one so the tracks still align,
        // a blank one holding the line's height for a week with nothing in it.
        Text(
            if (week.minutes > 0) "${week.minutes}" else " ",
            style = MaterialTheme.typography.labelSmall,
            color = if (isCurrent) onBg else muted,
            fontWeight = if (isCurrent) FontWeight.Bold else FontWeight.Normal
        )
        Spacer(Modifier.height(4.dp))
        Box(
            Modifier
                .fillMaxWidth()
                .height(WEEK_TRACK)
                // The target rule, drawn behind the bar on this track's own scale — no overlay to
                // keep in sync with the rows above it. A line as data (§1).
                .drawBehind {
                    val y = size.height * (1f - targetFraction)
                    drawLine(
                        color = ruleColor,
                        start = Offset(0f, y),
                        end = Offset(size.width, y),
                        strokeWidth = 1.dp.toPx(),
                        pathEffect = PathEffect.dashPathEffect(floatArrayOf(6f, 6f), 0f)
                    )
                },
            contentAlignment = Alignment.BottomCenter
        ) {
            BarGeomBox(
                geom = when {
                    // The week still running is honest about being unfinished rather than looking
                    // like a bad week.
                    isCurrent -> BarGeom(
                        height = (6 + (WEEK_TRACK.value - 6) * fraction).dp,
                        dashedOutline = accent
                    )
                    week.isEmpty -> BarGeom(height = 4.dp, fill = outline.copy(alpha = 0.35f))
                    else -> BarGeom(height = (6 + (WEEK_TRACK.value - 6) * fraction).dp, fill = accent)
                }
            )
        }
        Spacer(Modifier.height(8.dp))
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            color = if (isCurrent) onBg else muted,
            fontWeight = if (isCurrent) FontWeight.Bold else FontWeight.Normal
        )
    }
}
