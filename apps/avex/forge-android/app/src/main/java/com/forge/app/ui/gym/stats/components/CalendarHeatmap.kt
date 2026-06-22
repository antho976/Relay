package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.dp
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.temporal.TemporalAdjusters

/**
 * GitHub-contributions-style adherence grid: one cell per day (weeks as columns, Mon→Sun rows),
 * tinted by that day's set count. Glanceable "did I show up?" — the most-beloved consistency view.
 * Horizontally scrollable so a long history fits a phone. Future days in the current week render empty.
 */
@Composable
internal fun CalendarHeatmap(
    activityByDay: Map<Long, Int>,
    weeks: Int,
    faint: Color,
    accent: Color,
    modifier: Modifier = Modifier
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

    Row(
        modifier.horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(3.dp)
    ) {
        for (w in 0 until weeks) {
            Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                for (d in 0..6) {
                    val date = startMonday.plusWeeks(w.toLong()).plusDays(d.toLong())
                    val color = if (date.isAfter(today)) {
                        Color.Transparent
                    } else {
                        val sets = activityByDay[date.toEpochDay()] ?: 0
                        if (sets == 0) faint else lerp(faint, accent, sets.toFloat() / maxSets)
                    }
                    androidx.compose.foundation.layout.Box(
                        Modifier.size(11.dp).clip(RoundedCornerShape(2.dp)).background(color)
                    )
                }
            }
        }
    }
}
