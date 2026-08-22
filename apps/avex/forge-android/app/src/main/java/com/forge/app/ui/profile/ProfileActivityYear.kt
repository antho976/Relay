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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.theme.MonoSectionAnchor
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.YearMonth
import java.time.format.TextStyle
import java.util.Locale

/**
 * # ACTIVITY — the whole year, as a contribution graph
 *
 * GitHub's graph, at GitHub's proportions: **weekday down, week across**, seven rows of small
 * rounded squares running left to right from January to December, each lit by how many times you
 * trained that day. Month labels ride above the columns their month starts in; the key sits at the
 * bottom right.
 *
 * ## Month → year (2026-08-22, second pass)
 *
 * The first version of this section drew one month at 22dp cells. It was readable per-day and it
 * was wrong: at that size the block covered barely a third of the page width, so the section was a
 * small square with a lot of nothing beside it, and a month of training is too short a window to
 * show a shape. Antho, on seeing it: *"make it yearly, that looked better honestly."*
 *
 * The year has to earn its density instead. Fifty-three columns across the page put a day at about
 * 4.5dp, which is roughly what GitHub itself shows on a phone — small enough that no single day is
 * the point, and that is exactly the point. A year of training read at arm's length is a texture:
 * where the streaks are, where the gaps are, whether the back half of the year is denser than the
 * front. The two figures underneath answer the questions the texture cannot be counted for.
 *
 * ## Why the ramp is fixed, not normalized
 *
 * The old year grid normalized against the busiest day, so the same 1-session Tuesday changed shade
 * depending on whether you once did three-a-days in March. Training counts run 0–3; there are only
 * ever three rungs of meaning, so they are spent directly: trained, trained twice, trained more.
 * A fixed ramp means a lit square means the same thing in January and in August, and it is what the
 * key can honestly label.
 *
 * Days outside the year draw nothing, so the band keeps the year's own edges. Days still ahead of
 * today draw below the rest-day rung — the year keeps its full width without claiming a future day
 * is a rest day.
 */

/**
 * The gap between day cells. The cell itself is not a fixed size: 53 columns have to fit the page
 * exactly, so each column takes an equal share of what is left and the cell squares itself off that.
 */
private val CELL_GAP = 1.5.dp

/** GitHub's square, not the old grid's dot — the shape is half of why the mark reads as this one. */
private val CELL_SHAPE = RoundedCornerShape(1.5.dp)

/**
 * The key's swatch, and its own radius.
 *
 * It cannot reuse [CELL_SHAPE] or the cell's size: a day cell is ~4.5dp, and four of those in a row
 * would be a key too small to read the ramp off. The swatch is drawn at a legible size with a radius
 * in the same proportion, so it still reads as the same square.
 */
private val SWATCH = 11.dp
private val SWATCH_SHAPE = RoundedCornerShape(2.5.dp)

/**
 * Rest days. Deliberately above the 0.18 the old dot grid used: measured on device, 0.18 at this
 * cell size read as switched off, and a rest day is a real answer that should look like one.
 * A boundary tone, exempt from the text contrast floor (§14).
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
internal fun ProfileActivityYear(
    activityByDay: Map<Long, Int>,
    onBg: Color,
    muted: Color,
    hue: Color,
    modifier: Modifier = Modifier
) {
    val today = remember { LocalDate.now() }
    val year = today.year

    // The band spans whole weeks: back to the Monday on or before Jan 1, forward to the Sunday on or
    // after Dec 31. Monday-first matches the app's other calendar (Stats' heatmap). That is 53 weeks
    // most years and 54 when Jan 1 lands late in a leap year — computed, never assumed.
    val gridStart = remember(year) {
        val first = LocalDate.of(year, 1, 1)
        first.minusDays((first.dayOfWeek.value - DayOfWeek.MONDAY.value).toLong())
    }
    val weeks = remember(year, gridStart) {
        val last = LocalDate.of(year, 12, 31)
        val gridEnd = last.plusDays((DayOfWeek.SUNDAY.value - last.dayOfWeek.value).toLong())
        ((gridEnd.toEpochDay() - gridStart.toEpochDay() + 1) / 7).toInt()
    }

    // How many week-columns each month occupies, so its label can sit over its own stretch of the
    // band rather than over an even twelfth of it. February is four columns; a 31-day month that
    // straddles six weeks is six.
    val monthSpans = remember(year, gridStart, weeks) {
        val counts = IntArray(12)
        for (w in 0 until weeks) {
            // Attribute a column to the month its Thursday falls in — the ISO tiebreak, and the one
            // that stops a month label drifting a column early when the 1st lands on a Saturday.
            val thursday = gridStart.plusDays((w * 7 + 3).toLong())
            if (thursday.year == year) counts[thursday.monthValue - 1]++
        }
        counts.toList()
    }

    // Both readings come off the same map the old grid used — no new state on the ViewModel.
    val yearCounts = remember(activityByDay, year) {
        activityByDay.entries
            .filter { LocalDate.ofEpochDay(it.key).year == year && it.value > 0 }
            .map { it.value }
    }
    val activeDays = yearCounts.size
    val sessions = yearCounts.sum()

    val empty = MaterialTheme.colorScheme.outline.copy(alpha = EMPTY_ALPHA)
    val future = MaterialTheme.colorScheme.outline.copy(alpha = FUTURE_ALPHA)

    Column(modifier.fillMaxWidth()) {
        // Not `SectionAnchor`: its trailing slot is a navigation link ("view all →"), and the year
        // is a caption for what is drawn, not somewhere to go.
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("ACTIVITY", style = MonoSectionAnchor, color = muted)
            Text("$year", style = MaterialTheme.typography.labelSmall, color = muted, maxLines = 1)
        }
        Spacer(Modifier.height(12.dp))
        MonthLabels(monthSpans, muted)
        Spacer(Modifier.height(5.dp))
        YearBand(
            gridStart = gridStart,
            weeks = weeks,
            year = year,
            today = today,
            activityByDay = activityByDay,
            activeDays = activeDays,
            sessions = sessions,
            empty = empty,
            future = future,
            hue = hue
        )
        Spacer(Modifier.height(16.dp))
        // The two figures the band cannot be counted for, and the key to its ramp — one line, the
        // reading on the left where reading starts, the key at the right edge as GitHub places it.
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.Bottom
        ) {
            Row(verticalAlignment = Alignment.Bottom) {
                YearReading("$activeDays", "ACTIVE DAYS", onBg, muted)
                Spacer(Modifier.width(20.dp))
                YearReading("$sessions", if (sessions == 1) "SESSION" else "SESSIONS", onBg, muted)
            }
            RampLegend(empty, hue, muted)
        }
    }
}

/** JAN … DEC, each label weighted to the number of week-columns its month actually spans. */
@Composable
private fun MonthLabels(monthSpans: List<Int>, muted: Color) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(CELL_GAP)) {
        monthSpans.forEachIndexed { i, span ->
            if (span <= 0) return@forEachIndexed
            Text(
                YearMonth.of(2000, i + 1).month
                    .getDisplayName(TextStyle.SHORT, Locale.getDefault()).uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 8.sp,
                letterSpacing = 0.sp,
                maxLines = 1,
                modifier = Modifier.weight(span.toFloat())
            )
        }
    }
}

/** The band itself: one weighted column per week, seven square cells down each. */
@Composable
private fun YearBand(
    gridStart: LocalDate,
    weeks: Int,
    year: Int,
    today: LocalDate,
    activityByDay: Map<Long, Int>,
    activeDays: Int,
    sessions: Int,
    empty: Color,
    future: Color,
    hue: Color
) {
    val reading = if (activeDays == 0) {
        "$year: no sessions logged yet"
    } else {
        "$year: trained on $activeDays days, $sessions sessions"
    }
    Row(
        Modifier
            .fillMaxWidth()
            .semantics(mergeDescendants = true) { contentDescription = reading },
        horizontalArrangement = Arrangement.spacedBy(CELL_GAP)
    ) {
        for (week in 0 until weeks) {
            Column(
                Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(CELL_GAP)
            ) {
                for (row in 0..6) {
                    val date = gridStart.plusDays((week * 7 + row).toLong())
                    // `aspectRatio` off the weighted width is what squares the cell — the band's
                    // height is therefore a consequence of the page width, never a hardcoded number.
                    val cell = Modifier.fillMaxWidth().aspectRatio(1f)
                    if (date.year != year) {
                        Box(cell) // outside the year — nothing drawn, so the band keeps its edges
                    } else {
                        val level = levelOf(activityByDay[date.toEpochDay()] ?: 0)
                        val color = when {
                            date.isAfter(today) -> future
                            level == 0 -> empty
                            else -> lerp(empty, hue, LIT_RUNGS[level - 1])
                        }
                        Box(cell.clip(CELL_SHAPE).background(color))
                    }
                }
            }
        }
    }
}

@Composable
private fun YearReading(figure: String, noun: String, onBg: Color, muted: Color) {
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
 * because the ramp is ordinal and the band's own reading names both counts in words.
 */
@Composable
private fun RampLegend(empty: Color, hue: Color, muted: Color, modifier: Modifier = Modifier) {
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        Text("LESS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
        Spacer(Modifier.width(6.dp))
        Box(Modifier.size(SWATCH).clip(SWATCH_SHAPE).background(empty))
        LIT_RUNGS.forEach { rung ->
            Spacer(Modifier.width(3.dp))
            Box(Modifier.size(SWATCH).clip(SWATCH_SHAPE).background(lerp(empty, hue, rung)))
        }
        Spacer(Modifier.width(6.dp))
        Text("MORE", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
    }
}
