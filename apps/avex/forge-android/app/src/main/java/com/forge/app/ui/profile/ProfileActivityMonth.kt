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
import androidx.compose.foundation.layout.widthIn
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
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.theme.MonoSectionAnchor
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.YearMonth
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale

/**
 * # ACTIVITY — this month, as a calendar of contribution squares
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
 * Turned, a month IS a calendar: seven weekday columns, five or six week rows. Same squares, same
 * ramp, same key — the mark is unchanged, only its axes swap to suit the window.
 *
 * ## Edge to edge (2026-08-24)
 *
 * The grid spent 0.75 of the measure for a while, to buy back the height seven full-width columns
 * cost. On device that trade read as a bug, not as restraint: the anchor above it, the readings
 * line below it and the LESS/MORE key all run to the right margin, so a grid stopping a quarter
 * short is the one element on the page that fails to reach it — Antho pointed straight at the gap
 * ("make the activity reach the full side, there's empty space").
 *
 * So it takes the whole measure. The cells are square and sized off the width, which means width is
 * the only size control there is: full width puts a day near 42dp and the section near 295dp, and
 * that height is the honest price of a month drawn as squares rather than a number to be trimmed
 * back. It buys a calendar you can actually read a date off, on the same margins as everything
 * else on the page.
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
 * ## Tappable, and pageable (2026-08-31)
 *
 * It was deliberately passive at first, on the grounds that the tappable day-detail already lived
 * on Stats. In practice nobody goes to Stats to answer "what did I do on the 12th" while looking
 * straight at the 12th, so a lit day opens the same [com.forge.app.ui.common.DayLogSheet] Stats
 * opens — the same rows, drilling into the same session and cardio screens. Rest days stay inert.
 *
 * The month name became a stepper at the same time, because a calendar you cannot page is a
 * calendar that answers questions about this month only. Back stops at the oldest month with
 * anything in it, forward stops at this one.
 *
 * Swapping back to the year is one call site in [ProfileScreen]; both live in the package.
 */

/**
 * The gap between day cells. The cell itself is not a fixed size: seven columns have to fit the
 * page's measure exactly, so each takes an equal share of what is left and the cell squares itself
 * off that. Widening the gap therefore costs cell size rather than section height — the two trade
 * against each other and the grid's height stays ~6/7 of the measure either way.
 */
private val MONTH_CELL_GAP = 8.dp

/** Larger cells want a proportionally larger radius, or a 42dp square reads as a hard-edged tile. */
private val MONTH_CELL_SHAPE = RoundedCornerShape(9.dp)

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
    /** The run you are on right now, in days. Below 2 there is no run, and nothing is printed. */
    streakDays: Int,
    /** The longest run ever. Printed only when it beats [streakDays] — see [MonthStreak]. */
    longestStreakDays: Int,
    onBg: Color,
    muted: Color,
    hue: Color,
    /** Opens the day sheet. Only lit days call it — a rest day has nothing to show. */
    onDayTap: ((LocalDate) -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    val today = remember { LocalDate.now() }
    val thisMonth = remember(today) { YearMonth.from(today) }
    // How many months back the grid is showing. An Int, not a YearMonth, because that is what
    // survives process death through rememberSaveable without a custom Saver.
    var monthsBack by rememberSaveable { mutableIntStateOf(0) }
    val month = remember(thisMonth, monthsBack) { thisMonth.minusMonths(monthsBack.toLong()) }

    // How far back there is anything to look at. Paging into a run of empty months is a worse
    // answer than a dead arrow, and the map now spans all of history rather than this year, so it
    // knows where the record actually starts.
    val oldestMonth = remember(activityByDay, thisMonth) {
        activityByDay.keys.minOrNull()?.let { YearMonth.from(LocalDate.ofEpochDay(it)) } ?: thisMonth
    }
    val canGoBack = month.isAfter(oldestMonth)
    val canGoForward = monthsBack > 0

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
    // Both streak figures are about TODAY, not about the month on screen. Printed under a paged-back
    // March they would read as March's run, which is a wrong answer dressed as a reading — so they
    // show only while the grid is showing the month they belong to.
    val streak = remember(streakDays, longestStreakDays, monthsBack) {
        if (monthsBack == 0) MonthStreak.of(streakDays, longestStreakDays) else null
    }

    val empty = MaterialTheme.colorScheme.outline.copy(alpha = MONTH_EMPTY_ALPHA)
    val future = MaterialTheme.colorScheme.outline.copy(alpha = MONTH_FUTURE_ALPHA)
    val monthName = month.month.getDisplayName(TextStyle.FULL, Locale.getDefault()).uppercase()

    Column(modifier.fillMaxWidth()) {
        // Not `SectionAnchor`: its trailing slot is a navigation link ("view all →"), and the month
        // name is not somewhere to go — it is the caption for what is drawn, and now also the
        // readout of a stepper. Chevrons in `IconButton`s, matching Stats' heatmap pager exactly,
        // because it is the same control over the same kind of grid.
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("ACTIVITY", style = MonoSectionAnchor, color = muted)
            Row(verticalAlignment = Alignment.CenterVertically) {
                MonthStep(
                    icon = Icons.Filled.ChevronLeft,
                    label = "Earlier month",
                    enabled = canGoBack,
                    onBg = onBg,
                    muted = muted
                ) { monthsBack++ }
                Text(
                    "$monthName ${month.year}",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    textAlign = TextAlign.Center,
                    // A floor, not a fixed width: the label still grows for SEPTEMBER and still
                    // scales with the font setting. It stops the chevrons walking sideways under
                    // the finger that is tapping them as the month name changes length.
                    modifier = Modifier.widthIn(min = 108.dp)
                )
                MonthStep(
                    icon = Icons.Filled.ChevronRight,
                    label = "Later month",
                    enabled = canGoForward,
                    onBg = onBg,
                    muted = muted
                ) { monthsBack-- }
            }
        }
        Spacer(Modifier.height(14.dp))
        // Header and grid share one width so the weekday labels stay centred over their columns.
        Column(Modifier.fillMaxWidth()) {
            WeekdayHeader(muted)
            Spacer(Modifier.height(8.dp))
            // No `mergeDescendants` any more: it collapsed the whole month into one announcement,
            // which was right while the grid was a passive texture and is wrong now that the cells
            // are individually tappable. Each day speaks for itself below, and the summary the
            // merge used to carry is printed in words on the readings line further down.
            Column(
                Modifier.fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(MONTH_CELL_GAP)
            ) {
                for (week in 0 until weeks) {
                    Row(
                        Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(MONTH_CELL_GAP)
                    ) {
                        for (day in 0..6) {
                            val date = gridStart.plusDays((week * 7 + day).toLong())
                            // `aspectRatio` off the weighted width is what squares the cell — the
                            // grid's height is a consequence of its width, never a hardcoded number.
                            val cell = Modifier.weight(1f).aspectRatio(1f)
                            if (YearMonth.from(date) != month) {
                                // Outside the month — nothing drawn, so the grid keeps its shape.
                                Box(cell)
                            } else {
                                val count = activityByDay[date.toEpochDay()] ?: 0
                                val level = monthLevelOf(count)
                                val color = when {
                                    date.isAfter(today) -> future
                                    level == 0 -> empty
                                    else -> lerp(empty, hue, MONTH_LIT_RUNGS[level - 1])
                                }
                                // Only a LIT day opens the sheet — same rule as Stats' heatmap. A
                                // rest day has nothing to show, and a tap that opens "nothing
                                // logged" teaches the grid is broken rather than that you rested.
                                val tap = onDayTap.takeIf { count > 0 }
                                Box(
                                    cell
                                        .clip(MONTH_CELL_SHAPE)
                                        .background(color)
                                        .semantics { contentDescription = dayReading(date, count) }
                                        .then(
                                            if (tap != null)
                                                Modifier.clickableLabeled("See what you did") { tap(date) }
                                            else Modifier
                                        )
                                )
                            }
                        }
                    }
                }
            }
        }
        // The key goes under the grid it labels, on the right rail — NOT on the readings line
        // (Antho, 2026-08-24: "the less/more is not aligned here").
        //
        // The two were sharing a row, which asked an 11dp swatch strip to line up with a
        // `headlineSmall` figure and its 9sp caption. Bottom-aligning a Row aligns boxes, not
        // baselines, and the caption is pinned to an 18dp box that its 9sp glyphs sit high inside —
        // so the key hung off the bottom of that box, a few dp below everything it was meant to
        // agree with. No alignment rule fixes that pairing, because the two things do not belong on
        // one line to begin with: a reading is a figure you take off the page, a key is the axis of
        // the mark above it. Put back against the grid it explains, it needs no alignment at all,
        // and the readings get the whole measure for the streak that moved down off the cover.
        Spacer(Modifier.height(12.dp))
        MonthRampLegend(empty, hue, muted, Modifier.align(Alignment.End))
        Spacer(Modifier.height(18.dp))
        // The figures the grid cannot be counted for. BEST sits off at the right margin and only
        // when it beats the current run — printed beside an equal streak it is the same number
        // twice, which is what it was doing up on the cover photo.
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
                MonthReading("$activeDays", "ACTIVE DAYS", onBg, muted)
                MonthReading("$sessions", if (sessions == 1) "SESSION" else "SESSIONS", onBg, muted)
                streak?.let {
                    MonthReading(it.figure, it.noun, onBg, muted)
                }
            }
            streak?.best?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    // No `maxLines = 1` (§14): two mono words with the whole right margin to
                    // themselves have nothing to truncate for, and clamping them is how a caption
                    // silently loses its figure at a large font scale.
                )
            }
        }
    }
}

/**
 * The streak, resolved into what the readings line actually prints.
 *
 * Three states, and the rule behind them is that a figure earns its place by being news. An active
 * run is the live answer, so it leads. A best only appears alongside it when it is a DIFFERENT
 * number — a "BEST 2" next to a 2-day streak is the redundancy that made the cover's caption and
 * chip read as a duplicate. With no run going, the best is the only streak fact left, so it takes
 * the reading slot itself rather than vanishing.
 *
 * Under two days there is no streak in either sense: one session is a session, not a run.
 */
private data class MonthStreak(val figure: String, val noun: String, val best: String?) {
    companion object {
        fun of(streakDays: Int, longestStreakDays: Int): MonthStreak? = when {
            streakDays >= 2 -> MonthStreak(
                "$streakDays", "DAY STREAK",
                best = "BEST $longestStreakDays".takeIf { longestStreakDays > streakDays }
            )
            longestStreakDays >= 2 -> MonthStreak("$longestStreakDays", "BEST STREAK", best = null)
            else -> null
        }
    }
}

/**
 * One chevron of the month stepper. A full [IconButton] rather than a bare tinted glyph, for the
 * 48dp target §14 asks for; at a bound it dims and goes inert rather than disappearing, so the
 * stepper keeps its shape and the reason you cannot page further is visible.
 */
@Composable
private fun MonthStep(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    enabled: Boolean,
    onBg: Color,
    muted: Color,
    onClick: () -> Unit
) {
    IconButton(onClick = onClick, enabled = enabled) {
        Icon(icon, contentDescription = label, tint = if (enabled) onBg else muted.copy(alpha = 0.35f))
    }
}

/** What TalkBack says on one day cell: the date, then what is on it. */
private fun dayReading(date: LocalDate, count: Int): String {
    val day = date.format(DAY_READING_FMT)
    return when {
        count <= 0 -> "$day, rest day"
        count == 1 -> "$day, 1 session"
        else -> "$day, $count sessions"
    }
}

private val DAY_READING_FMT: DateTimeFormatter =
    DateTimeFormatter.ofPattern("MMMM d", Locale.getDefault())

/**
 * M T W T F S S, each centred over its column.
 *
 * The year band labels only Mon/Wed/Fri down its left edge, because seven labels beside a 9dp row
 * would not fit. Turned, every column is ~42dp wide and there is room for all seven — and a
 * calendar missing four of its weekday headers reads as broken rather than as restrained.
 *
 * On the scale's own `labelMedium` rung, not a hand-set 9sp. The 9sp was sized for the 28dp column
 * the grid had at 0.75 of the measure; over a full-width column it read as a stray mark rather than
 * as the axis of the grid beneath it. Taking the rung whole also puts the labels back under the
 * user's font-size setting, which a call-site `fontSize` opts out of (§6).
 */
@Composable
private fun WeekdayHeader(muted: Color) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(MONTH_CELL_GAP)) {
        for (day in 1..7) {
            Text(
                DayOfWeek.of(day).getDisplayName(TextStyle.NARROW, Locale.getDefault()).uppercase(),
                style = MaterialTheme.typography.labelMedium,
                color = muted,
                textAlign = TextAlign.Center,
                modifier = Modifier.weight(1f)
            )
        }
    }
}

@Composable
private fun MonthReading(figure: String, noun: String, onBg: Color, muted: Color) {
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
 * because the ramp is ordinal and the grid's own reading names both counts in words.
 */
@Composable
private fun MonthRampLegend(empty: Color, hue: Color, muted: Color, modifier: Modifier = Modifier) {
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        Text("LESS", style = MaterialTheme.typography.labelSmall, color = muted)
        Spacer(Modifier.width(6.dp))
        Box(Modifier.size(MONTH_SWATCH).clip(MONTH_SWATCH_SHAPE).background(empty))
        MONTH_LIT_RUNGS.forEach { rung ->
            Spacer(Modifier.width(3.dp))
            Box(Modifier.size(MONTH_SWATCH).clip(MONTH_SWATCH_SHAPE).background(lerp(empty, hue, rung)))
        }
        Spacer(Modifier.width(6.dp))
        Text("MORE", style = MaterialTheme.typography.labelSmall, color = muted)
    }
}
