package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
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
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.theme.MonoSectionAnchor
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.YearMonth
import java.time.format.TextStyle

/**
 * # ACTIVITY — the whole year, as a contribution graph
 *
 * GitHub's graph, at GitHub's proportions: **weekday down, week across**, seven rows of rounded
 * squares each lit by how many times you trained that day. Month labels ride above the columns
 * their month starts in; the key sits at the bottom right.
 *
 * ## The year is wrapped onto two bands
 *
 * GitHub draws all 53 weeks on one line because it has a desktop's width to spend. On a phone that
 * line puts a day at ~4.5dp — an honest texture, and too small to be the page's activity section.
 * Antho, on seeing it: *"make activity bigger."*
 *
 * There is no way to make a 53-column single row bigger; the width is the width. So the year wraps:
 * January–June on the first band, July–December on the second, ~27 columns each, which doubles the
 * cell to ~9dp and quadruples the section's presence. Both bands are laid out over the SAME number
 * of slots — the shorter half pads with empty ones at its tail — so a day is exactly the same size
 * on both, which is the whole reason the wrap does not read as two unrelated charts.
 *
 * ## Month → year (2026-08-22, second pass)
 *
 * The first version of this section drew one month at 22dp cells. It was readable per-day and it
 * was wrong: at that size the block covered barely a third of the page width, so the section was a
 * small square with a lot of nothing beside it, and a month of training is too short a window to
 * show a shape. Antho, on seeing it: *"make it yearly, that looked better honestly."*
 *
 * A year read at arm's length is a texture rather than a table: where the streaks are, where the
 * gaps are, whether the back half of the year is denser than the front. No single day is the point.
 * The two figures underneath answer the questions the texture cannot be counted for.
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
 * The gap between day cells. The cell itself is not a fixed size: a half-year of columns has to fit
 * the page exactly, so each column takes an equal share of what is left and the cell squares itself
 * off that. At ~9dp a 2.5dp gap is the ratio GitHub uses at its own cell size.
 */
private val CELL_GAP = 2.5.dp

/** GitHub's square, not the old grid's dot — the shape is half of why the mark reads as this one. */
private val CELL_SHAPE = RoundedCornerShape(2.5.dp)

/** The air between the two half-year bands — wider than a cell gap, so they read as two lines. */
private val BAND_GAP = 16.dp

/**
 * The key's swatch, and its own radius.
 *
 * It cannot reuse the cell's size: a day is ~9dp and shrinks with the page, so four of those in a
 * row would be a key that changes size on a narrower phone. The swatch is drawn at a fixed legible
 * size, sharing [CELL_SHAPE]'s radius so it still reads as the same square.
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

    // Which half-year each week-column belongs to, and how many columns each month occupies inside
    // its half — so a label sits over its own stretch of the band rather than over an even sixth of
    // it. February is four columns; a 31-day month that straddles six weeks is six.
    val halves = remember(year, gridStart, weeks) {
        val firstSpans = IntArray(6)
        val secondSpans = IntArray(6)
        var firstCols = 0
        var secondCols = 0
        for (w in 0 until weeks) {
            // Attribute a column to the month its Thursday falls in — the ISO tiebreak, and the one
            // that stops a month label drifting a column early when the 1st lands on a Saturday.
            // A padding week whose Thursday escapes the year is attributed to the half it abuts.
            val thursday = gridStart.plusDays((w * 7 + 3).toLong())
            val month = when {
                thursday.year < year -> 1
                thursday.year > year -> 12
                else -> thursday.monthValue
            }
            if (month <= 6) { firstSpans[month - 1]++; firstCols++ }
            else { secondSpans[month - 7]++; secondCols++ }
        }
        listOf(
            HalfYear(startWeek = 0, spans = firstSpans.toList(), firstMonth = 1),
            HalfYear(startWeek = firstCols, spans = secondSpans.toList(), firstMonth = 7)
        ) to maxOf(firstCols, secondCols)
    }
    val (bands, slots) = halves

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
            Text("$year", style = MaterialTheme.typography.labelSmall, color = muted)
        }
        Spacer(Modifier.height(14.dp))
        bands.forEachIndexed { i, band ->
            if (i > 0) Spacer(Modifier.height(BAND_GAP))
            MonthLabels(band, slots, muted)
            Spacer(Modifier.height(6.dp))
            YearBand(
                band = band,
                slots = slots,
                gridStart = gridStart,
                year = year,
                today = today,
                activityByDay = activityByDay,
                // One reading on the first band names the whole year; the second is decorative to a
                // screen reader, because announcing the same summary twice is worse than silence.
                reading = if (i == 0) yearReading(year, activeDays, sessions) else null,
                empty = empty,
                future = future,
                hue = hue
            )
        }
        Spacer(Modifier.height(18.dp))
        // The two figures the band cannot be counted for, and the key to its ramp — one line, the
        // reading on the left where reading starts, the key at the right edge as GitHub places it.
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.Bottom
        ) {
            FlowRow(
                modifier = Modifier.weight(1f),
                horizontalArrangement = Arrangement.spacedBy(20.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                itemVerticalAlignment = Alignment.Bottom
            ) {
                YearReading("$activeDays", "ACTIVE DAYS", onBg, muted)
                YearReading("$sessions", if (sessions == 1) "SESSION" else "SESSIONS", onBg, muted)
            }
            RampLegend(empty, hue, muted)
        }
    }
}

/** One half-year of week-columns: where it starts, and how many columns each of its months owns. */
private data class HalfYear(val startWeek: Int, val spans: List<Int>, val firstMonth: Int) {
    val columns: Int get() = spans.sum()
}

/** The year's reading, spoken once for both bands. */
private fun yearReading(year: Int, activeDays: Int, sessions: Int): String =
    if (activeDays == 0) "$year: no sessions logged yet"
    else "$year: trained on $activeDays days, $sessions sessions"

/**
 * JAN … JUN (or JUL … DEC), each label weighted to the number of week-columns its month spans, over
 * a row of [slots] so both bands' labels sit above the same column pitch.
 */
@Composable
private fun MonthLabels(band: HalfYear, slots: Int, muted: Color) {
    val locale = currentLocale()
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(CELL_GAP)) {
        band.spans.forEachIndexed { i, span ->
            if (span <= 0) return@forEachIndexed
            Text(
                YearMonth.of(2000, band.firstMonth + i).month
                    .getDisplayName(TextStyle.NARROW, locale).uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                modifier = Modifier.weight(span.toFloat())
            )
        }
        // Pad the shorter half so its labels keep the other's pitch instead of stretching to fill.
        val pad = slots - band.columns
        if (pad > 0) Spacer(Modifier.weight(pad.toFloat()))
    }
}

/** One band: a weighted column per week of this half, seven square cells down each. */
@Composable
private fun YearBand(
    band: HalfYear,
    slots: Int,
    gridStart: LocalDate,
    year: Int,
    today: LocalDate,
    activityByDay: Map<Long, Int>,
    reading: String?,
    empty: Color,
    future: Color,
    hue: Color
) {
    Row(
        Modifier
            .fillMaxWidth()
            .then(
                if (reading != null) {
                    Modifier.semantics(mergeDescendants = true) { contentDescription = reading }
                } else Modifier
            ),
        horizontalArrangement = Arrangement.spacedBy(CELL_GAP)
    ) {
        for (col in 0 until band.columns) {
            Column(
                Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(CELL_GAP)
            ) {
                for (row in 0..6) {
                    val date = gridStart.plusDays(((band.startWeek + col) * 7 + row).toLong())
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
        // Same pad as the labels: the shorter half ends early rather than drawing wider days.
        val pad = slots - band.columns
        if (pad > 0) Spacer(Modifier.weight(pad.toFloat()))
    }
}

@Composable
private fun YearReading(figure: String, noun: String, onBg: Color, muted: Color) {
    Row(verticalAlignment = Alignment.Bottom) {
        Text(figure, style = MaterialTheme.typography.headlineSmall, color = onBg)
        Spacer(Modifier.width(5.dp))
        Text(
            noun,
            style = MaterialTheme.typography.labelSmall,
            color = muted
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
        Text("LESS", style = MaterialTheme.typography.labelSmall, color = muted)
        Spacer(Modifier.width(6.dp))
        Box(Modifier.size(SWATCH).clip(SWATCH_SHAPE).background(empty))
        LIT_RUNGS.forEach { rung ->
            Spacer(Modifier.width(3.dp))
            Box(Modifier.size(SWATCH).clip(SWATCH_SHAPE).background(lerp(empty, hue, rung)))
        }
        Spacer(Modifier.width(6.dp))
        Text("MORE", style = MaterialTheme.typography.labelSmall, color = muted)
    }
}
