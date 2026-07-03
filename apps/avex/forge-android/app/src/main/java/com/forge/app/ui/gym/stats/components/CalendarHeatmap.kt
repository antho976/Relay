package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ChevronLeft
import androidx.compose.material.icons.filled.ChevronRight
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.bounceClick
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.temporal.TemporalAdjusters

/** How many weeks (columns) one page of the heatmap shows; [HEATMAP_WEEKS] divides evenly by this. */
private const val WEEKS_PER_PAGE = 13
private val RANGE_FMT = DateTimeFormatter.ofPattern("d MMM")

/**
 * GitHub-contributions-style adherence grid: one cell per day (weeks as columns, Mon→Sun rows),
 * tinted by that day's set count. Glanceable "did I show up?" — the most-beloved consistency view.
 * Paged with chevron arrows a page ([WEEKS_PER_PAGE] weeks) at a time rather than free-scrolled — a
 * horizontal scroll nested in the vertically-scrolling Stats page fought the finger — and opens on
 * the newest page (the one holding today). When [onDayTap] is set, lit days are tappable and open
 * that day's detail.
 */
@Composable
internal fun CalendarHeatmap(
    activityByDay: Map<Long, Int>,
    weeks: Int,
    faint: Color,
    accent: Color,
    modifier: Modifier = Modifier,
    onDayTap: ((LocalDate) -> Unit)? = null
) {
    val today = remember { LocalDate.now() }
    val startMonday = remember(today, weeks) {
        today.with(TemporalAdjusters.previousOrSame(DayOfWeek.MONDAY)).minusWeeks((weeks - 1).toLong())
    }
    // Normalize intensity against the busiest day IN THE VISIBLE WINDOW, not all-time — otherwise one
    // huge day from before the window washes every recent day out to near-faint.
    val maxSets = remember(activityByDay, startMonday, today) {
        val window = startMonday.toEpochDay()..today.toEpochDay()
        (activityByDay.entries.filter { it.key in window }.maxOfOrNull { it.value } ?: 1).coerceAtLeast(1)
    }

    val weeksPerPage = WEEKS_PER_PAGE.coerceAtMost(weeks)
    val pageCount = (weeks + weeksPerPage - 1) / weeksPerPage
    // pagesBack: 0 = the newest page (the one containing today); higher = further into the past.
    var pagesBack by rememberSaveable { mutableStateOf(0) }
    val safeBack = pagesBack.coerceIn(0, pageCount - 1)
    val lastWeek = weeks - 1 - safeBack * weeksPerPage
    val firstWeek = lastWeek - weeksPerPage + 1 // may be < 0 on the oldest page — those columns pad out blank
    val canGoOlder = safeBack < pageCount - 1
    val canGoNewer = safeBack > 0

    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val rangeLabel = remember(firstWeek, lastWeek, startMonday, today) {
        val start = startMonday.plusWeeks(firstWeek.coerceAtLeast(0).toLong())
        val end = startMonday.plusWeeks(lastWeek.toLong()).plusDays(6).let { if (it.isAfter(today)) today else it }
        "${RANGE_FMT.format(start)} – ${RANGE_FMT.format(end)}".uppercase()
    }

    Column(modifier) {
        // Page navigator — chevrons flank the visible date range; dimmed + inert at the ends.
        Row(
            Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            IconButton(onClick = { if (canGoOlder) pagesBack = safeBack + 1 }, enabled = canGoOlder) {
                Icon(Icons.Filled.ChevronLeft, contentDescription = "Earlier weeks", tint = if (canGoOlder) onBg else faint)
            }
            Text(rangeLabel, style = MaterialTheme.typography.labelSmall, color = muted, letterSpacing = 1.sp)
            IconButton(onClick = { if (canGoNewer) pagesBack = safeBack - 1 }, enabled = canGoNewer) {
                Icon(Icons.Filled.ChevronRight, contentDescription = "Later weeks", tint = if (canGoNewer) onBg else faint)
            }
        }
        Spacer(Modifier.height(8.dp))
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            for (col in 0 until weeksPerPage) {
                val weekIdx = firstWeek + col
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    for (d in 0..6) {
                        // Cells fill the page width evenly (weight) and stay square, so a page always
                        // spans the card no matter the screen. Off-range columns pad out invisibly.
                        val cell = Modifier.fillMaxWidth().aspectRatio(1f)
                        if (weekIdx < 0) {
                            Box(cell)
                        } else {
                            val date = startMonday.plusWeeks(weekIdx.toLong()).plusDays(d.toLong())
                            val sets = activityByDay[date.toEpochDay()] ?: 0
                            // Future days (the rest of the current week) get the same faint fill as a
                            // rest day rather than rendering as an empty hole — the missing-tile look
                            // read as a bug. Matches the contributions-grid convention.
                            val color = if (sets == 0 || date.isAfter(today)) faint
                                        else lerp(faint, accent, sets.toFloat() / maxSets)
                            Box(
                                cell
                                    .clip(RoundedCornerShape(4.dp))
                                    .background(color)
                                    .then(
                                        if (onDayTap != null && sets > 0)
                                            Modifier.bounceClick { onDayTap(date) }
                                        else Modifier
                                    )
                            )
                        }
                    }
                }
            }
        }
    }
}
