package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.theme.MonoSectionAnchor
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.YearMonth
import java.time.format.TextStyle
import java.util.Locale

/**
 * # ACTIVITY — this month, at full width
 *
 * The same contribution-grid vocabulary as [ProfileActivityYear] — rounded squares, one fixed ramp,
 * a LESS/MORE key — scoped to the current calendar month and turned ninety degrees.
 *
 * ## Why the month turns and the year does not
 *
 * A contribution graph is weekday-down, week-across because a year is 53 weeks long and only seven
 * days tall: that orientation is what lets a year fit a screen at all. A month is the opposite
 * shape — six weeks by seven days — so borrowing the year's orientation puts six columns on a page
 * with room for twenty-six, which is exactly what the first attempt did and why it read as a small
 * square with nothing beside it.
 *
 * Turned, a month IS a calendar: seven weekday columns, five or six week rows, filling the page
 * edge to edge. Same squares, same ramp, same key — the mark is unchanged, only its axes swap to
 * suit the window. A day lands at roughly 38dp, which is the largest any version of this section
 * has drawn one.
 *
 * ## Why the ramp is fixed, not normalized
 *
 * The old year grid normalized against the busiest day, so the same 1-session Tuesday changed shade
 * depending on whether you once did three-a-days in March. Training counts run 0–3; there are only
 * ever three rungs of meaning, so they are spent directly: trained, trained twice, trained more.
 * A fixed ramp means a lit square means the same thing in January and in August, and it is what the
 * key can honestly label.
 *
 * Days outside the month draw nothing, so the grid keeps the month's own ragged first and last
 * rows. Days still ahead of today draw below the rest-day rung — the month keeps its full shape
 * without claiming a future day is a rest day.
 *
 * Swapping back to the year is one call site in [ProfileScreen]; both live in the package.
 */

/**
 * The gap between day cells. The cell itself is not a fixed size: seven columns have to fit the page
 * exactly, so each takes an equal share of what is left and the cell squares itself off that. The
 * gap is wider than the year's because the cells it separates are four times the size.
 */
private val MONTH_CELL_GAP = 8.dp

/** Larger cells want a proportionally larger radius, or a 38dp square reads as a hard-edged tile. */
private val MONTH_CELL_SHAPE = RoundedCornerShape(8.dp)

/** The key's swatch. Fixed, unlike the cells, so the key does not resize with the phone. */
private val MONTH_SWATCH = 11.dp
private val MONTH_SWATCH_SHAPE = RoundedCornerShape(3.dp)

/**
 * Rest days. Deliberately above the 0.18 the old dot grid used: measured on device, 0.18 read as
 * switched off, and a rest day is a real answer that should look like one. A boundary tone, exempt
 * from the text contrast floor (§14).
 */
private const val MONTH_EMPTY_ALPHA = 0.30f

/** Days that have not happened yet: present, and clearly below the rest-day rung. */
private const val MONTH_FUTURE_ALPHA = 0.13f

/** The three lit rungs: trained, trained twice, trained more. Indexed by [monthLevelOf] minus one. */
private val MONTH_LIT_RUNGS = floatArrayOf(0.52f, 0.76f, 1f)

/** 0 for a rest day, else 1–3. Fixed thresholds — see the file header. */
private fun monthLevelOf(count: Int): Int = when {
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

    val empty = MaterialTheme.colorScheme.outline.copy(alpha = MONTH_EMPTY_ALPHA)
    val future = MaterialTheme.colorScheme.outline.copy(alpha = MONTH_FUTURE_ALPHA)
    val monthName = month.month.getDisplayName(TextStyle.FULL, Locale.getDefault()).uppercase()
    val reading = if (activeDays == 0) {
        "$monthName: no sessions logged yet"
    } else {
        "$monthName: trained on $activeDays days, $sessions sessions"
    }

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
        WeekdayHeader(muted)
        Spacer(Modifier.height(8.dp))
        Column(
            Modifier
                .fillMaxWidth()
                .semantics(mergeDescendants = true) { contentDescription = reading },
            verticalArrangement = Arrangement.spacedBy(MONTH_CELL_GAP)
        ) {
            for (week in 0 until weeks) {
                Row(
                    Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(MONTH_CELL_GAP)
                ) {
                    for (day in 0..6) {
                        val date = gridStart.plusDays((week * 7 + day).toLong())
                        // `aspectRatio` off the weighted width is what squares the cell — the grid's
                        // height is a consequence of the page width, never a hardcoded number.
                        val cell = Modifier.weight(1f).aspectRatio(1f)
                        if (YearMonth.from(date) != month) {
                            Box(cell) // outside the month — nothing drawn, so the grid keeps its shape
                        } else {
                            val level = monthLevelOf(activityByDay[date.toEpochDay()] ?: 0)
                            val color = when {
                                date.isAfter(today) -> future
                                level == 0 -> empty
                                else -> lerp(empty, hue, MONTH_LIT_RUNGS[level - 1])
                            }
                            Box(cell.clip(MONTH_CELL_SHAPE).background(color))
                        }
                    }
                }
            }
        }
        Spacer(Modifier.height(18.dp))
        // The two figures the grid cannot be counted for, and the key to its ramp — one line, the
        // reading on the left where reading starts, the key at the right edge as GitHub places it.
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.Bottom
        ) {
            Row(verticalAlignment = Alignment.Bottom) {
                MonthReading("$activeDays", "ACTIVE DAYS", onBg, muted)
                Spacer(Modifier.width(20.dp))
                MonthReading("$sessions", if (sessions == 1) "SESSION" else "SESSIONS", onBg, muted)
            }
            MonthRampLegend(empty, hue, muted)
        }
    }
}

/**
 * M T W T F S S, each centred over its column.
 *
 * The year band labels only Mon/Wed/Fri down its left edge, because seven labels beside a 9dp row
 * would not fit. Turned, every column is 38dp wide and there is room for all seven — and a calendar
 * missing four of its weekday headers reads as broken rather than as restrained.
 */
@Composable
private fun WeekdayHeader(muted: Color) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(MONTH_CELL_GAP)) {
        for (day in 1..7) {
            Text(
                DayOfWeek.of(day).getDisplayName(TextStyle.NARROW, Locale.getDefault()).uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 9.sp,
                maxLines = 1,
                textAlign = TextAlign.Center,
                modifier = Modifier.weight(1f)
            )
        }
    }
}

@Composable
private fun MonthReading(figure: String, noun: String, onBg: Color, muted: Color) {
    Row(verticalAlignment = Alignment.Bottom) {
        Text(figure, style = MaterialTheme.typography.headlineSmall, color = onBg, maxLines = 1)
        Spacer(Modifier.width(5.dp))
        Text(
            noun,
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontSize = 9.sp,
            maxLines = 1,
            modifier = Modifier.height(18.dp)
        )
    }
}

/**
 * LESS ▫▪▪▪ MORE — the swatch row from the contribution graph, showing the four rungs actually
 * drawn above. It is a key to a colour ramp, which is the one thing §14 lets colour carry alone,
 * because the ramp is ordinal and the grid's own reading names both counts in words.
 */
@Composable
private fun MonthRampLegend(empty: Color, hue: Color, muted: Color, modifier: Modifier = Modifier) {
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        Text("LESS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
        Spacer(Modifier.width(6.dp))
        Box(Modifier.size(MONTH_SWATCH).clip(MONTH_SWATCH_SHAPE).background(empty))
        MONTH_LIT_RUNGS.forEach { rung ->
            Spacer(Modifier.width(3.dp))
            Box(Modifier.size(MONTH_SWATCH).clip(MONTH_SWATCH_SHAPE).background(lerp(empty, hue, rung)))
        }
        Spacer(Modifier.width(6.dp))
        Text("MORE", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
    }
}
