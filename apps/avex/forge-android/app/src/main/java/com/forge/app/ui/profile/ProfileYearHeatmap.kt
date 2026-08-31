package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.time.LocalDate
import java.time.YearMonth
import java.time.format.TextStyle
import java.util.Locale

/** Fixed gutter for the mono month label so day-of-month columns line up across every row. */
private val MONTH_LABEL_WIDTH = 26.dp

/** 31 columns so day-of-month aligns vertically; short months pad their trailing days out blank. */
private const val DAYS_WIDE = 31

/**
 * THIS YEAR — a full calendar year of consistency at a glance: one ROW per month (Jan→Dec), one dot
 * per day (day-of-month is the column), each dot lit by how many times you trained that day (gym
 * sessions + cardio). Rest days and future days sit faint so the year reads as a filling grid.
 *
 * Deliberately a DIFFERENT mark from the Stats adherence heatmap — that one is week-columns, a
 * rolling 26-week window, gym set-load, and tappable; this is the whole calendar year, keyed to
 * "did I show up" across all training, and passive. So the two consistency views don't echo each
 * other (§4.3). A passive glance like the lifetime-volume sparkline — the tappable day-detail already
 * lives on Stats at a comfortable cell size. The caller hides this section when the year is empty.
 */
@Composable
internal fun YearConsistencySection(
    activityByDay: Map<Long, Int>,
    muted: Color,
    accent: Color,
    modifier: Modifier = Modifier
) {
    val today = remember { LocalDate.now() }
    val year = today.year
    // Both readings are about THIS YEAR, and the map they read spans all of history — it stopped at
    // the year boundary until ACTIVITY needed to page back through months. Scope them here rather
    // than at the source: the grid draws one year and is the only thing that knows which.
    val thisYear = remember(activityByDay, year) {
        activityByDay.filterKeys { LocalDate.ofEpochDay(it).year == year }
    }
    // Normalize against the busiest day so the gradient spreads; a 0.5 floor keeps a single-workout
    // day clearly lit (workout counts run low, so raw normalizing would wash lone days out to faint).
    val maxCount = remember(thisYear) { (thisYear.values.maxOrNull() ?: 1).coerceAtLeast(1) }
    val activeDays = thisYear.size
    val faint = MaterialTheme.colorScheme.outline.copy(alpha = 0.18f)

    Column(modifier) {
        SectionHeader("THIS YEAR", muted)
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            for (m in 1..12) {
                val len = YearMonth.of(year, m).lengthOfMonth()
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        LocalDate.of(year, m, 1).month
                            .getDisplayName(TextStyle.SHORT, Locale.getDefault()).uppercase(),
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp,
                        maxLines = 1,
                        modifier = Modifier.width(MONTH_LABEL_WIDTH)
                    )
                    Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                        for (day in 1..DAYS_WIDE) {
                            val cell = Modifier.weight(1f).aspectRatio(1f)
                            if (day > len) {
                                Box(cell) // pad so day-of-month columns stay aligned across months
                            } else {
                                val date = LocalDate.of(year, m, day)
                                val count = activityByDay[date.toEpochDay()] ?: 0
                                val color = if (count == 0 || date.isAfter(today)) faint
                                    else lerp(faint, accent, 0.5f + 0.5f * (count.toFloat() / maxCount))
                                Box(cell.clip(CircleShape).background(color))
                            }
                        }
                    }
                }
            }
        }
        Spacer(Modifier.height(10.dp))
        // Mono is the uppercase micro-label voice (§6) — a reading joined with · , not a sentence.
        Text(
            "$activeDays ACTIVE DAYS · $year",
            style = MaterialTheme.typography.labelSmall,
            color = muted, fontSize = 9.sp, letterSpacing = 1.sp
        )
    }
}
