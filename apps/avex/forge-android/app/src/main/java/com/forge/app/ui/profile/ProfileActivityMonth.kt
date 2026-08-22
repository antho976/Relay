package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.theme.MonoSectionAnchor
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.YearMonth
import java.time.format.TextStyle
import java.util.Locale

/**
 * # ACTIVITY — this month, as a contribution grid
 *
 * The GitHub graph's exact geometry, scoped to one calendar month: **weekday down, week across**.
 * Seven rows (Mon→Sun), one column per week the month touches, one rounded square per day, lit by
 * how many times you trained that day. It replaces the 12-row THIS YEAR dot grid (2026-08-22), and
 * the trade is deliberate: a year of 4dp dots was a texture you could admire but not read, where a
 * month at this cell size answers "how has THIS month gone" — the only consistency question you can
 * still act on. The year view is still in the package ([YearConsistencySection]) and uncalled.
 *
 * ## Why the ramp is fixed, not normalized
 *
 * The year grid normalized against the busiest day, so the same 1-session Tuesday changed shade
 * depending on whether you once did three-a-days in March. Training counts run 0–3; there are only
 * ever three rungs of meaning, so they are spent directly: trained, trained twice, trained more.
 * A fixed ramp means a lit square means the same thing in January and in August, and it is what the
 * legend below the grid can honestly label.
 *
 * Days outside the month draw nothing, which gives the block GitHub's staircase corners. Days still
 * ahead of today draw at half the empty rung — the month keeps its full rectangle without claiming
 * a future day is a rest day.
 */

/** One day. Fixed dp: the cell carries no text, so §14's "size to content" does not apply. */
private val CELL = 22.dp
private val CELL_GAP = 5.dp

/** GitHub's square, not the year grid's dot — the shape is half of why the mark reads as this one. */
private val CELL_SHAPE = RoundedCornerShape(5.dp)

/**
 * The legend's swatch, and its own radius.
 *
 * It cannot reuse [CELL_SHAPE]: a 5dp radius on a 10dp box is a circle, and the first build shipped
 * a row of dots explaining a grid of squares — a key that does not look like the thing it keys.
 */
private val SWATCH = 12.dp
private val SWATCH_SHAPE = RoundedCornerShape(3.dp)

/** The weekday gutter. Fits "M" at the mono micro rung with the grid clear of it. */
private val WEEKDAY_GUTTER = 16.dp

/**
 * Rest days. The year grid's 0.18 rung was inherited and then measured on device: at 4dp it was a
 * texture, at 22dp it was a barely-there smudge that made the whole block read as switched off. A
 * rest day is a real answer and should look like one. Boundary tone, exempt from the text contrast
 * floor (§14).
 */
private const val EMPTY_ALPHA = 0.30f

/** Days that have not happened yet: present, and clearly below the rest-day rung. */
private const val FUTURE_ALPHA = 0.13f

/** The three lit rungs: trained, trained twice, trained more. Indexed by [levelOf] minus one. */
private val LIT_RUNGS = floatArrayOf(0.52f, 0.76f, 1f)

/** 0 for a rest day, else 1–3. Fixed thresholds — see the file header. */
private fun levelOf(count: Int): Int = when {
    count <= 0 -> 0
    count == 1 -> 1
    count == 2 -> 2
    else -> 3
}

@Composable
internal fun ProfileActivityMonth(
    activityByDay: Map<Long, Int>,
    onBg: Color,
    muted: Color,
    hue: Color,
    modifier: Modifier = Modifier
) {
    val today = remember { LocalDate.now() }
    val month = remember(today) { YearMonth.from(today) }

    // The grid spans whole weeks: back to the Monday on or before the 1st, forward to the Sunday on
    // or after the last day. Monday-first matches the app's other calendar (Stats' heatmap).
    val gridStart = remember(month) {
        val first = month.atDay(1)
        first.minusDays((first.dayOfWeek.value - DayOfWeek.MONDAY.value).toLong())
    }
    val weeks = remember(month, gridStart) {
        val last = month.atEndOfMonth()
        val gridEnd = last.plusDays((DayOfWeek.SUNDAY.value - last.dayOfWeek.value).toLong())
        ((gridEnd.toEpochDay() - gridStart.toEpochDay() + 1) / 7).toInt()
    }

    // Both readings come off the same map the year grid used — no new state on the ViewModel.
    val monthCounts = remember(activityByDay, month) {
        (1..month.lengthOfMonth())
            .mapNotNull { activityByDay[month.atDay(it).toEpochDay()] }
            .filter { it > 0 }
    }
    val activeDays = monthCounts.size
    val sessions = monthCounts.sum()

    val empty = MaterialTheme.colorScheme.outline.copy(alpha = EMPTY_ALPHA)
    val future = MaterialTheme.colorScheme.outline.copy(alpha = FUTURE_ALPHA)
    val monthName = month.month.getDisplayName(TextStyle.FULL, Locale.getDefault()).uppercase()

    Column(modifier.fillMaxWidth()) {
        // Not `SectionAnchor`: its trailing slot is a navigation link ("view all →"), and the month
        // name is a caption for what is drawn, not somewhere to go.
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("ACTIVITY", style = MonoSectionAnchor, color = muted)
            Text(
                "$monthName ${month.year}",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                maxLines = 1
            )
        }
        Spacer(Modifier.height(14.dp))

        Row(verticalAlignment = Alignment.Top) {
            MonthGrid(
                gridStart = gridStart,
                weeks = weeks,
                month = month,
                today = today,
                activityByDay = activityByDay,
                activeDays = activeDays,
                sessions = sessions,
                monthName = monthName,
                empty = empty,
                future = future,
                hue = hue,
                muted = muted
            )
            Spacer(Modifier.width(18.dp))
            // The two readings the grid cannot be counted for at a glance. Figures on `onBg`, their
            // nouns muted — the same pairing the all-time rows use, so the page keeps one voice.
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                MonthReading("$activeDays", "ACTIVE DAYS", onBg, muted)
                MonthReading("$sessions", if (sessions == 1) "SESSION" else "SESSIONS", onBg, muted)
            }
        }

        Spacer(Modifier.height(12.dp))
        // Indented to the first column of cells, not right-aligned to the page. GitHub can put its
        // key at the far right because its graph spans the full width; this grid stops well short of
        // it, and a key floating in the gap beside nothing belongs to nothing.
        RampLegend(empty, hue, muted, Modifier.padding(start = WEEKDAY_GUTTER + CELL_GAP))
    }
}

/** The grid itself plus its Mon/Wed/Fri gutter — GitHub labels exactly those three rows. */
@Composable
private fun MonthGrid(
    gridStart: LocalDate,
    weeks: Int,
    month: YearMonth,
    today: LocalDate,
    activityByDay: Map<Long, Int>,
    activeDays: Int,
    sessions: Int,
    monthName: String,
    empty: Color,
    future: Color,
    hue: Color,
    muted: Color
) {
    val reading = if (activeDays == 0) {
        "$monthName: no sessions logged yet"
    } else {
        "$monthName: trained on $activeDays days, $sessions sessions"
    }
    Row(
        Modifier.semantics(mergeDescendants = true) { contentDescription = reading },
        horizontalArrangement = Arrangement.spacedBy(CELL_GAP)
    ) {
        // Weekday gutter. Rows 0/2/4 are Mon/Wed/Fri; the rest stay blank so the labels never crowd
        // the 15dp cells they sit beside.
        Column(
            Modifier.width(WEEKDAY_GUTTER),
            verticalArrangement = Arrangement.spacedBy(CELL_GAP)
        ) {
            for (row in 0..6) {
                Box(Modifier.height(CELL), contentAlignment = Alignment.CenterStart) {
                    if (row % 2 == 0 && row <= 4) {
                        Text(
                            DayOfWeek.of(row + 1)
                                .getDisplayName(TextStyle.NARROW, Locale.getDefault()).uppercase(),
                            style = MaterialTheme.typography.labelSmall,
                            color = muted,
                            fontSize = 9.sp,
                            maxLines = 1
                        )
                    }
                }
            }
        }
        for (week in 0 until weeks) {
            Column(verticalArrangement = Arrangement.spacedBy(CELL_GAP)) {
                for (row in 0..6) {
                    val date = gridStart.plusDays((week * 7 + row).toLong())
                    if (YearMonth.from(date) != month) {
                        // Outside the month — nothing drawn, so the block keeps the month's shape.
                        Spacer(Modifier.size(CELL))
                    } else {
                        val level = levelOf(activityByDay[date.toEpochDay()] ?: 0)
                        val color = when {
                            date.isAfter(today) -> future
                            level == 0 -> empty
                            else -> lerp(empty, hue, LIT_RUNGS[level - 1])
                        }
                        Box(Modifier.size(CELL).clip(CELL_SHAPE).background(color))
                    }
                }
            }
        }
    }
}

@Composable
private fun MonthReading(figure: String, noun: String, onBg: Color, muted: Color) {
    Column {
        Text(figure, style = MaterialTheme.typography.headlineSmall, color = onBg, maxLines = 1)
        Spacer(Modifier.height(2.dp))
        Text(noun, style = MaterialTheme.typography.labelSmall, color = muted, maxLines = 1)
    }
}

/**
 * LESS ▫▪▪▪ MORE — the swatch row from the contribution graph, showing the four rungs actually
 * drawn above. It is a key to a colour ramp, which is the one thing §14 lets colour carry alone,
 * because the ramp is ordinal and the grid's own reading names both counts in words.
 */
@Composable
private fun RampLegend(empty: Color, hue: Color, muted: Color, modifier: Modifier = Modifier) {
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        Text("LESS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
        Spacer(Modifier.width(7.dp))
        Box(Modifier.size(SWATCH).clip(SWATCH_SHAPE).background(empty))
        LIT_RUNGS.forEach { rung ->
            Spacer(Modifier.width(4.dp))
            Box(Modifier.size(SWATCH).clip(SWATCH_SHAPE).background(lerp(empty, hue, rung)))
        }
        Spacer(Modifier.width(7.dp))
        Text("MORE", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
    }
}
