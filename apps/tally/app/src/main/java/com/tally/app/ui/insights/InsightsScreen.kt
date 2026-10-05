@file:OptIn(ExperimentalLayoutApi::class, ExperimentalMaterial3Api::class)

package com.tally.app.ui.insights

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.rounded.ShowChart
import androidx.compose.material.icons.automirrored.rounded.TrendingDown
import androidx.compose.material.icons.automirrored.rounded.TrendingUp
import androidx.compose.material.icons.rounded.CalendarMonth
import androidx.compose.material.icons.rounded.CalendarToday
import androidx.compose.material.icons.rounded.Category
import androidx.compose.material.icons.rounded.Event
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material.icons.rounded.MoreHoriz
import androidx.compose.material.icons.rounded.Savings
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.Aside
import com.tally.app.ui.common.Caption
import com.tally.app.ui.accounts.typeLabel
import com.tally.app.ui.common.CategoryBadge
import com.tally.app.ui.common.CategoryIcons
import com.tally.app.ui.common.ChromeButton
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.EndsRow
import com.tally.app.ui.common.FIGURE_GAP
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LedgerRow
import com.tally.app.ui.common.LegendDot
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.Panel
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.ROW_FOCUS_OUTSET
import com.tally.app.ui.common.SlidingSegments
import com.tally.app.ui.common.StackedBar
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.StatTile
import com.tally.app.ui.common.ThinBar
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.common.bounceClick
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.nav.FAB_CLEARANCE
import com.tally.app.ui.theme.categoryColor
import com.tally.core.Copy
import com.tally.core.MoneyFormatter
import com.tally.core.PaceStatus
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.format.TextStyle
import java.time.temporal.ChronoUnit
import java.util.Locale
import kotlin.math.roundToInt

/**
 * The Insights tab. The lens is UI state, kept here so it survives a tab switch; the period and
 * every reading come from the ViewModel. A period row in the Trend lens opens that month.
 */
@Composable
fun InsightsTab(nav: AppNav) {
    val viewModel: InsightsViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    var lens by rememberSaveable { mutableStateOf(InsightsLens.MONTH) }
    LifecycleResumeEffect(viewModel) {
        viewModel.onResume()
        onPauseOrDispose { }
    }
    InsightsScreen(
        state = state,
        lens = lens,
        actions = InsightsActions(
            onLens = { lens = it },
            previousPeriod = viewModel::previousPeriod,
            nextPeriod = viewModel::nextPeriod,
            openPeriod = { offset ->
                viewModel.showPeriod(offset)
                lens = InsightsLens.MONTH
            },
            openCategory = { nav.transactions(categoryId = it) },
            openEntry = { nav.entry(id = it) },
        ),
    )
}

/** Everything Insights can do, as plain lambdas, so the screen renders in a test with no graph. */
data class InsightsActions(
    val onLens: (InsightsLens) -> Unit = {},
    val previousPeriod: () -> Unit = {},
    val nextPeriod: () -> Unit = {},
    /** Opens the period [offset] cycles from the current one in the Month lens. */
    val openPeriod: (Int) -> Unit = {},
    val openCategory: (Long) -> Unit = {},
    val openEntry: (Long) -> Unit = {},
)

/**
 * Insights in four lenses. Month: the spend line against its pace in the lit panel, the readings
 * around it, where the money went, the largest entries, the biggest payees and what came in. Trend:
 * six periods side by side and their averages. Calendar: the days as a heat grid, each one opening
 * its entries. Worth: what the accounts add up to, held against owed, and what was kept and invested.
 */
@Composable
fun InsightsScreen(state: InsightsState, lens: InsightsLens, actions: InsightsActions) {
    val periodName = remember(state.period, state.today) { Dates.period(state.period, state.today) }
    val lenses = remember { InsightsLens.entries.map { it.label } }
    var openDay by rememberSaveable { mutableStateOf<Long?>(null) }
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = FAB_CLEARANCE),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        item(key = "head") {
            Column {
                TopBar(onBack = null) {
                    PeriodStepper(
                        label = periodName,
                        canGoNext = !state.isCurrentPeriod,
                        onPrevious = actions.previousPeriod,
                        onNext = actions.nextPeriod,
                    )
                }
                val context = if (state.isCurrentPeriod) {
                    "$periodName · day ${state.elapsedDays} of ${state.period.days}"
                } else {
                    "$periodName · " + Copy.plural(state.period.days, "day")
                }
                PageTitle(
                    "Insights",
                    Modifier.padding(horizontal = GUTTER).padding(top = 4.dp, bottom = 6.dp),
                    context = context,
                )
            }
        }
        item(key = "lens") {
            SlidingSegments(
                options = lenses,
                selectedIndex = lens.ordinal,
                onSelect = { actions.onLens(InsightsLens.entries[it]) },
                modifier = Modifier.padding(horizontal = GUTTER).fillMaxWidth(),
            )
        }
        // Before the first read every reading is a placeholder zero; "Nothing spent in October"
        // over a month of entries would be untrue, so only the frame shows until it lands.
        if (state.loaded) {
            when (lens) {
                InsightsLens.MONTH -> monthItems(state, periodName, actions)
                InsightsLens.TREND -> trendItems(state, actions)
                InsightsLens.CALENDAR -> calendarItems(state, periodName) { openDay = it.toEpochDay() }
                InsightsLens.WORTH -> worthItems(state, periodName)
            }
        }
    }
    val day = openDay
    if (state.loaded && lens == InsightsLens.CALENDAR && day != null) {
        DaySheet(
            date = LocalDate.ofEpochDay(day),
            state = state,
            onEntry = { id ->
                openDay = null
                actions.openEntry(id)
            },
            onDismiss = { openDay = null },
        )
    }
}

// ── Shared pieces ───────────────────────────────────────────────────────────────────────────────

/**
 * The period stepper in the top bar's chrome: previous, the period's mono name, next. There is no
 * next past the current period; a spacer holds the label still instead.
 */
@Composable
private fun PeriodStepper(label: String, canGoNext: Boolean, onPrevious: () -> Unit, onNext: () -> Unit) {
    ChromeButton(Icons.AutoMirrored.Rounded.KeyboardArrowLeft, "Previous month", onPrevious)
    Text(
        label.uppercase(),
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.onBackground,
        textAlign = TextAlign.Center,
    )
    if (canGoNext) {
        ChromeButton(Icons.AutoMirrored.Rounded.KeyboardArrowRight, "Next month", onNext)
    } else {
        Spacer(Modifier.size(48.dp))
    }
}

/** The lit panel's label row: a glyph tile, the mono name of the reading, a mono reading at the end. */
@Composable
private fun LensHeroLabel(label: String, meta: String?, tint: Color = MaterialTheme.colorScheme.primary) {
    PanelHeader(label, meta = meta, tint = tint)
}

/**
 * The lit panel's serif figure. Past 1.5x font scale it steps down a rung of the same serif voice,
 * so an amount stays on one line instead of breaking between its digits.
 */
@Composable
private fun HeroFigure(text: String) {
    val large = LocalDensity.current.fontScale > 1.5f
    Text(
        text,
        style = if (large) MaterialTheme.typography.headlineLarge else MaterialTheme.typography.displayLarge,
        color = MaterialTheme.colorScheme.onBackground,
    )
}

/** Two different readings side by side, stacked once the text is too large for two columns. */
@Composable
private fun TileRow(
    modifier: Modifier = Modifier,
    first: @Composable (Modifier) -> Unit,
    second: @Composable (Modifier) -> Unit,
) {
    if (LocalDensity.current.fontScale > 1.5f) {
        Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(FIGURE_GAP)) {
            first(Modifier.fillMaxWidth())
            second(Modifier.fillMaxWidth())
        }
    } else {
        Row(modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(FIGURE_GAP)) {
            first(Modifier.weight(1f).fillMaxHeight())
            second(Modifier.weight(1f).fillMaxHeight())
        }
    }
}

/** A section's zero: one quiet line. */
@Composable
private fun EmptyLine(text: String, modifier: Modifier = Modifier) {
    Aside(text, modifier.fillMaxWidth())
}

@Composable
private fun ChipRow(chips: List<Pair<ImageVector, String>>) {
    if (chips.isEmpty()) return
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        chips.forEach { (icon, text) -> StatChip(icon, text) }
    }
}

private fun signedWhole(money: MoneyFormatter, v: Long): String = (if (v > 0L) "+" else "") + money.formatWhole(v)

private fun percentOf(part: Long, whole: Long): Int =
    if (whole <= 0L) 0 else (part.toDouble() * 100.0 / whole).roundToInt()

private fun shareText(part: Long, whole: Long): String {
    val p = percentOf(part, whole)
    return if (part > 0L && p == 0) "<1%" else "$p%"
}

/** "today", "yesterday", or the weekday and date, for a sentence that reads "By …". */
private fun dayWord(date: LocalDate, today: LocalDate): String = when (date) {
    today -> "today"
    today.minusDays(1) -> "yesterday"
    else -> Dates.day(date, today)
}

/** An empty day sheet's line: "today" and "yesterday" read as words, any other day by its date. */
internal fun nothingLoggedLine(date: LocalDate, today: LocalDate): String = when (date) {
    today -> "Nothing logged today"
    today.minusDays(1) -> "Nothing logged yesterday"
    else -> "Nothing logged on " + Dates.day(date, today)
}

private fun nothingSpentLine(state: InsightsState, periodName: String): String =
    if (state.isCurrentPeriod) "Nothing spent in $periodName yet" else "Nothing spent in $periodName"

private fun weekdayName(day: DayOfWeek): String {
    val locale = Locale.getDefault()
    return day.getDisplayName(TextStyle.FULL_STANDALONE, locale).replaceFirstChar { it.titlecase(locale) }
}

// ── Month ───────────────────────────────────────────────────────────────────────────────────────

private fun LazyListScope.monthItems(state: InsightsState, periodName: String, actions: InsightsActions) {
    item(key = "month-hero") { MonthHero(state, Modifier.padding(horizontal = GUTTER)) }
    item(key = "month-tiles") { MonthTiles(state, Modifier.padding(horizontal = GUTTER)) }
    item(key = "month-where") { WherePanel(state, periodName, actions.openCategory, Modifier.padding(horizontal = GUTTER)) }
    item(key = "month-largest") { LargestPanel(state, periodName, actions.openEntry, Modifier.padding(horizontal = GUTTER)) }
    if (state.payees.isNotEmpty()) {
        item(key = "month-payees") { PayeesPanel(state, Modifier.padding(horizontal = GUTTER)) }
    }
    item(key = "month-income") { IncomePanel(state, periodName, actions.openCategory, Modifier.padding(horizontal = GUTTER)) }
}

/**
 * The month's spend as the hero figure, and its line against the even pace. Dragging across the
 * line reads any day so far; untouched, the readout is today (the last day of a past period).
 */
@Composable
private fun MonthHero(state: InsightsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val r = state.reading
    val over = r?.status == PaceStatus.OVER_BUDGET
    val tint = if (over) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary
    var scrub by remember(state.period) { mutableStateOf<Int?>(null) }
    val drawn = state.cumulative.size
    val last = (drawn - 1).coerceAtLeast(0)
    val shown = (scrub ?: last).coerceIn(0, last)
    val days = state.period.days

    val meta = when {
        r != null && state.isCurrentPeriod -> Copy.paceLine(r, money)
        state.isCurrentPeriod -> "DAY ${state.elapsedDays} OF $days"
        else -> Copy.plural(days, "day").uppercase()
    }
    val supporting = when {
        r == null -> "across " + Copy.plural(state.elapsedDays, "day") + " · no budget set"
        over -> money.formatWhole(-r.remaining) + " over a " + money.formatWhole(r.budget) + " budget"
        state.isCurrentPeriod -> "of " + money.formatWhole(r.budget) + " budget · " + money.formatWhole(r.remaining) + " left"
        else -> "of " + money.formatWhole(r.budget) + " budget · " + money.formatWhole(r.remaining) + " to spare"
    }
    val readout = if (drawn == 0) {
        "Nothing to read yet"
    } else {
        val date = state.period.start.plusDays(shown.toLong())
        "By " + dayWord(date, state.today) + ": " + money.formatWhole(state.cumulative[shown]) + " spent" +
            (state.budget?.let { " · pace " + money.formatWhole(paceAt(it, shown, days)) } ?: "")
    }
    val description = if (drawn == 0) {
        "No days to chart yet"
    } else {
        val lastDate = state.period.start.plusDays(last.toLong())
        "Spent " + money.formatWhole(state.cumulative[last]) + " by " + Dates.short(lastDate, state.today) +
            (state.budget?.let { b ->
                " against a pace of " + money.formatWhole(paceAt(b, last, days)) + ". Budget " + money.formatWhole(b)
            } ?: "")
    }
    val top = maxOf(state.cumulative.lastOrNull() ?: 0L, state.budget ?: 0L)
    val chips = buildList {
        if (state.isCurrentPeriod) {
            if (r != null && !over) {
                add(Icons.Rounded.Speed to Copy.marginLine(r, money))
            } else {
                add(Icons.Rounded.CalendarToday to Copy.plural(state.daysLeft, "day") + " left")
            }
            state.projected?.let { add(Icons.AutoMirrored.Rounded.TrendingUp to "On track for " + money.formatWhole(it)) }
        } else {
            add(
                Icons.Rounded.Flag to when {
                    r == null -> "No budget set"
                    over -> "Ended over budget"
                    else -> "Ended within budget"
                },
            )
        }
    }

    HeroPanel(modifier) {
        LensHeroLabel(if (state.isCurrentPeriod) "Spent so far" else "Spent", meta, tint)
        Spacer(Modifier.height(14.dp))
        HeroFigure(money.formatWhole(state.spent))
        Text(supporting, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(18.dp))
        Text(readout, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onBackground)
        Spacer(Modifier.height(8.dp))
        MonthLine(
            cumulative = state.cumulative,
            days = days,
            budget = state.budget,
            selected = scrub?.coerceIn(0, last),
            onSelect = { scrub = it },
            description = description,
            startLabel = Dates.short(state.period.start, state.today).uppercase(),
            endLabel = Dates.short(state.period.lastDay, state.today).uppercase(),
            maxLabel = money.formatCompact(top),
        )
        Spacer(Modifier.height(12.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            LegendDot(MaterialTheme.colorScheme.primary, "Spent")
            if (state.budget != null) {
                LegendDot(MaterialTheme.colorScheme.onSurfaceVariant, "Even pace")
                LegendDot(MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.45f), "Budget")
            }
        }
        Spacer(Modifier.height(16.dp))
        ChipRow(chips)
    }
}

/** The readings around the month: per day, against last month, what was kept, and how many entries. */
@Composable
private fun MonthTiles(state: InsightsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val days = state.period.days
    val previousName = remember(state.period, state.today) { Dates.period(state.period.shift(-1), state.today) }
    val perDayDetail = state.budget?.let { "Budget allows " + money.formatWhole(it / days) + " a day" }
        ?: ("Over " + Copy.plural(state.elapsedDays, "day"))
    val versus = if (state.isCurrentPeriod) {
        Copy.versusLastLine(state.spent, state.lastPeriodSameDay, money)
    } else {
        val then = state.lastPeriodSameDay
        when {
            then <= 0L -> if (state.spent == 0L) "Nothing spent either month" else "Nothing spent in $previousName"
            state.spent == then -> "Level with $previousName"
            state.spent > then -> money.formatWhole(state.spent - then) + " more than $previousName"
            else -> money.formatWhole(then - state.spent) + " less than $previousName"
        }
    }
    val net = state.net
    Column(modifier, verticalArrangement = Arrangement.spacedBy(FIGURE_GAP)) {
        TileRow(
            first = { m ->
                StatTile("Per day", money.formatWhole(state.perDay), m, detail = perDayDetail)
            },
            second = { m ->
                StatTile(
                    if (state.isCurrentPeriod) "Last month" else "Month before",
                    money.formatWhole(state.lastPeriodSameDay),
                    m,
                    detail = versus,
                )
            },
        )
        TileRow(
            first = { m ->
                StatTile(
                    "Net",
                    signedWhole(money, net),
                    m,
                    detail = money.formatWhole(state.income) + " in · " + money.formatWhole(state.spent) + " out",
                )
            },
            second = { m ->
                StatTile(
                    "Expenses",
                    state.expenseCount.toString(),
                    m,
                    detail = if (state.expenseCount > 0) {
                        "Averaging " + money.formatWhole(state.spent / state.expenseCount) + " each"
                    } else if (state.isCurrentPeriod) "Nothing logged yet" else "Nothing logged",
                )
            },
        )
    }
}

/** Where the month went: the shares as one bar, then each category ranked with its own thin bar. */
@Composable
private fun WherePanel(state: InsightsState, periodName: String, onCategory: (Long) -> Unit, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val neutral = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f)
    val slices = state.categories
    val segments = remember(slices, neutral) {
        slices.map { it.total.toFloat() to (if (it.isFolded) neutral else categoryColor(it.color)) }
    }
    val description = remember(slices, state.spent, money) {
        if (slices.isEmpty()) "Nothing spent yet"
        else "Where it went: " + slices.joinToString(", ") { it.name + " " + shareText(it.total, state.spent) }
    }
    val top = slices.firstOrNull()?.total ?: 0L
    Panel(modifier) {
        PanelHeader(
            "Where it went",
            meta = if (slices.isEmpty()) null else Copy.plural(state.categoryCount, "category", "categories").uppercase(),
        )
        Spacer(Modifier.height(12.dp))
        StackedBar(segments, description, height = 12.dp)
        if (slices.isEmpty()) {
            Spacer(Modifier.height(14.dp))
            EmptyLine(nothingSpentLine(state, periodName))
        } else {
            Spacer(Modifier.height(4.dp))
            slices.forEach { s ->
                key(s.key) {
                    val id = s.categoryId
                    SliceRow(
                        slice = s,
                        share = shareText(s.total, state.spent),
                        fraction = if (top > 0L) (s.total.toFloat() / top).coerceIn(0f, 1f) else 0f,
                        amount = money.formatWhole(s.total),
                        amountColor = MaterialTheme.colorScheme.onBackground,
                        onClick = if (id != null && !s.isFolded) ({ onCategory(id) }) else null,
                    )
                }
            }
        }
    }
}

/**
 * One ranked category, the Stats-per-lift look: badge, name, mono entry count and share, the amount
 * at the end, and a thin bar in the category's hue under it. Folded and uncategorized rows open
 * nothing, so they take no tap.
 */
@Composable
private fun SliceRow(
    slice: CategorySlice,
    share: String,
    fraction: Float?,
    amount: String,
    amountColor: Color,
    onClick: (() -> Unit)?,
) {
    val hue = categoryColor(slice.color)
    val neutral = MaterialTheme.colorScheme.onSurfaceVariant
    val meta = buildList {
        if (slice.isFolded) add(Copy.plural(slice.folded, "category", "categories"))
        add(Copy.plural(slice.count, "entry", "entries"))
        add(share)
    }.joinToString(" · ")
    Column(
        Modifier
            .fillMaxWidth()
            .then(
                if (onClick != null) {
                    Modifier.bounceClick(label = "${slice.name} entries", focusOutset = ROW_FOCUS_OUTSET, onClick = onClick)
                } else {
                    Modifier
                },
            )
            .semantics(mergeDescendants = true) { contentDescription = "${slice.name}, $amount, $meta" }
            .heightIn(min = 56.dp)
            .padding(top = 12.dp, bottom = 6.dp),
    ) {
        Column(Modifier.clearAndSetSemantics { }) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                when {
                    slice.isFolded -> GlyphBadge(Icons.Rounded.MoreHoriz, tint = neutral, size = 36.dp)
                    slice.isUncategorized -> GlyphBadge(Icons.Rounded.Category, tint = hue, fill = hue.copy(alpha = 0.15f), size = 36.dp)
                    else -> CategoryBadge(slice.icon, slice.color, size = 36.dp)
                }
                EndsRow(
                    start = {
                        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            Text(slice.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                            Text(meta.uppercase(), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    },
                    end = { Text(amount, style = MaterialTheme.typography.titleSmall, color = amountColor) },
                    modifier = Modifier.weight(1f),
                )
            }
            if (fraction != null) {
                Spacer(Modifier.height(10.dp))
                ThinBar(
                    fraction,
                    if (slice.isFolded) neutral.copy(alpha = 0.6f) else hue,
                    description = "$share of spending",
                )
            }
        }
    }
}

@Composable
private fun LargestPanel(state: InsightsState, periodName: String, onEntry: (Long) -> Unit, modifier: Modifier = Modifier) {
    val rows = state.largest
    Panel(modifier) {
        PanelHeader(
            "Largest",
            meta = if (rows.isEmpty()) null else shareText(state.largestTotal, state.spent) + " OF SPENDING",
        )
        if (rows.isEmpty()) {
            Spacer(Modifier.height(10.dp))
            EmptyLine(
                if (state.isCurrentPeriod) "No spending to rank in $periodName yet" else "No spending to rank in $periodName",
            )
        } else {
            Spacer(Modifier.height(4.dp))
            rows.forEach { row ->
                key(row.id) { LedgerRow(row, Dates.day(row.date, state.today), onClick = { onEntry(row.id) }) }
            }
        }
    }
}

/** Who the money went to, by the entries' notes: the period's biggest payees and their share. */
@Composable
private fun PayeesPanel(state: InsightsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    Panel(modifier) {
        PanelHeader("Payees", meta = "TOP " + state.payees.size)
        Spacer(Modifier.height(4.dp))
        state.payees.forEach { p ->
            key(p.note) {
                val amount = money.formatWhole(p.total)
                val meta = Copy.plural(p.count, "entry", "entries") + " · " + shareText(p.total, state.spent) + " of spending"
                // A ranked comparison: each payee's share of the period's spending as a thin bar.
                Column(
                    Modifier
                        .fillMaxWidth()
                        .semantics(mergeDescendants = true) { contentDescription = "${p.note}, $amount, $meta" }
                        .padding(top = 10.dp, bottom = 6.dp),
                ) {
                    EndsRow(
                        start = {
                            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                Text(p.note, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                                Text(meta, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        },
                        end = {
                            Text(amount, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground, textAlign = TextAlign.End)
                        },
                        modifier = Modifier.clearAndSetSemantics { },
                    )
                    Spacer(Modifier.height(8.dp))
                    ThinBar(
                        if (state.spent > 0L) p.total.toFloat() / state.spent else 0f,
                        MaterialTheme.colorScheme.primary,
                        description = shareText(p.total, state.spent) + " of spending",
                    )
                }
            }
        }
    }
}

@Composable
private fun IncomePanel(state: InsightsState, periodName: String, onCategory: (Long) -> Unit, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val sources = state.incomeSources
    val inColor = MaterialTheme.colorScheme.tertiary
    Panel(modifier) {
        PanelHeader(
            "Income",
            meta = if (sources.isEmpty()) null else signedWhole(money, state.income),
            tint = inColor,
        )
        if (sources.isEmpty()) {
            Spacer(Modifier.height(10.dp))
            EmptyLine(
                if (state.isCurrentPeriod) "No income logged in $periodName yet" else "No income logged in $periodName",
            )
        } else {
            sources.forEach { s ->
                key(s.key) {
                    val id = s.categoryId
                    SliceRow(
                        slice = s,
                        share = shareText(s.total, state.income),
                        fraction = null,
                        amount = signedWhole(money, s.total),
                        amountColor = inColor,
                        onClick = if (id != null && !s.isFolded) ({ onCategory(id) }) else null,
                    )
                }
            }
        }
    }
}

// ── Trend ───────────────────────────────────────────────────────────────────────────────────────

private fun LazyListScope.trendItems(state: InsightsState, actions: InsightsActions) {
    item(key = "trend-hero") { TrendHero(state, Modifier.padding(horizontal = GUTTER)) }
    item(key = "trend-tiles") { TrendTiles(state, Modifier.padding(horizontal = GUTTER)) }
    item(key = "trend-list") { TrendListPanel(state, actions.openPeriod, Modifier.padding(horizontal = GUTTER)) }
}

/** Six periods side by side: the average month as the figure, the bars, and the picked month read out. */
@Composable
private fun TrendHero(state: InsightsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val points = state.trend
    val t = state.trendSummary
    val n = points.size
    var selected by rememberSaveable(state.offset, n) { mutableIntStateOf(n - 1) }
    val sel = selected.coerceIn(0, (n - 1).coerceAtLeast(0))
    val names = remember(points, state.today) { points.map { Dates.period(it.period, state.today) } }
    val labels = remember(points) { points.map { Dates.monthShort(it.period.start) } }
    val spent = remember(points) { points.map { it.spent } }
    val income = remember(points) { points.map { it.income } }
    val description = remember(points, names, money) {
        if (points.isEmpty()) "No months to chart"
        else "By month: " + points.indices.joinToString("; ") { i ->
            names[i] + " spent " + money.formatWhole(points[i].spent) + ", income " + money.formatWhole(points[i].income)
        }
    }
    val picked = points.getOrNull(sel)
    val empty = t.months == 0
    val supporting = when {
        empty -> "spent a month on average"
        t.partial -> "spent so far, the only month with entries"
        else -> "spent a month on average · " + Copy.plural(t.months, "full month")
    }
    val chips = buildList {
        // A running month is half a month: set against full ones it would always read "below".
        if (picked != null && !picked.running && !empty && !t.partial && (picked.spent > 0L || picked.income > 0L)) {
            val diff = picked.spent - t.averageSpent
            val name = names[sel]
            when {
                diff > 0L -> add(Icons.AutoMirrored.Rounded.TrendingUp to "$name " + money.formatWhole(diff) + " above average")
                diff < 0L -> add(Icons.AutoMirrored.Rounded.TrendingDown to "$name " + money.formatWhole(-diff) + " below average")
                else -> add(Icons.AutoMirrored.Rounded.TrendingUp to "$name on the average")
            }
        }
        val b = state.budget
        if (b != null) {
            add(Icons.Rounded.Flag to "Budget " + money.formatWhole(b))
            if (!empty) {
                add(
                    Icons.Rounded.Speed to if (t.overBudget > 0) "Over budget in ${t.overBudget} of $n" else "Within budget every month",
                )
            }
        }
    }
    HeroPanel(modifier) {
        LensHeroLabel("Average month", "LAST $n MONTHS")
        Spacer(Modifier.height(14.dp))
        HeroFigure(money.formatWhole(t.averageSpent))
        Text(supporting, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(18.dp))
        if (picked != null) {
            Text(
                names[sel] + ": spent " + money.formatWhole(picked.spent) + " · income " + money.formatWhole(picked.income),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onBackground,
            )
            Spacer(Modifier.height(8.dp))
        }
        TrendBars(
            spent = spent,
            income = income,
            labels = labels,
            names = names,
            current = n - 1,
            selected = sel,
            onSelect = { selected = it },
            budget = state.budget,
            description = description,
        )
        Spacer(Modifier.height(10.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            LegendDot(MaterialTheme.colorScheme.primary, "Spent")
            LegendDot(MaterialTheme.colorScheme.onBackground, "Income")
            if (state.budget != null) LegendDot(MaterialTheme.colorScheme.onSurfaceVariant, "Budget")
        }
        if (empty) {
            Spacer(Modifier.height(14.dp))
            EmptyLine("Nothing logged in the last six months")
        } else {
            Spacer(Modifier.height(16.dp))
            ChipRow(chips)
        }
    }
}

@Composable
private fun TrendTiles(state: InsightsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val t = state.trendSummary
    val points = state.trend
    val high = t.highest?.let { points.getOrNull(it) }
    val low = t.lowest?.let { points.getOrNull(it) }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(FIGURE_GAP)) {
        TileRow(
            first = { m ->
                StatTile(
                    "Highest month",
                    money.formatWhole(high?.spent ?: 0L),
                    m,
                    detail = high?.let { Dates.period(it.period, state.today) + if (it.running) " so far" else "" } ?: "Nothing spent yet",
                )
            },
            second = { m ->
                StatTile(
                    "Lowest month",
                    money.formatWhole(low?.spent ?: 0L),
                    m,
                    detail = low?.let { Dates.period(it.period, state.today) } ?: "Needs two full months",
                )
            },
        )
        TileRow(
            first = { m ->
                StatTile(
                    "Average income",
                    money.formatWhole(t.averageIncome),
                    m,
                    detail = if (t.months > 0) "Over " + Copy.plural(t.months, "month") else "Nothing logged yet",
                    valueColor = MaterialTheme.colorScheme.tertiary,
                )
            },
            second = { m ->
                StatTile(
                    "Average net",
                    signedWhole(money, t.averageNet),
                    m,
                    detail = "Income less spending, a month",
                )
            },
        )
    }
}

/** The six periods as rows, newest first; each opens that month in the Month lens. */
@Composable
private fun TrendListPanel(state: InsightsState, onOpen: (Int) -> Unit, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val newestFirst = remember(state.trend) { state.trend.asReversed() }
    Panel(modifier) {
        PanelHeader("Month by month", meta = if (newestFirst.isEmpty()) null else "NEWEST FIRST")
        if (newestFirst.isEmpty()) {
            Spacer(Modifier.height(10.dp))
            EmptyLine("Nothing logged in the last six months")
        } else {
            Spacer(Modifier.height(4.dp))
            newestFirst.forEach { p ->
                key(p.offset) {
                    val name = Dates.period(p.period, state.today) + if (p.running) " so far" else ""
                    val meta = "In " + money.formatWhole(p.income) + " · net " + signedWhole(money, p.net)
                    val spent = money.formatWhole(p.spent)
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .bounceClick(label = "Open $name", focusOutset = ROW_FOCUS_OUTSET) { onOpen(p.offset) }
                            .semantics(mergeDescendants = true) { contentDescription = "$name, spent $spent, $meta" }
                            .heightIn(min = 60.dp)
                            .padding(vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        GlyphBadge(
                            Icons.Rounded.CalendarMonth,
                            tint = if (p.offset == state.offset) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onBackground,
                            size = 36.dp,
                        )
                        EndsRow(
                            start = {
                                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                    Text(name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                                    Text(meta.uppercase(), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            },
                            end = { Text(spent, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground) },
                            modifier = Modifier.weight(1f).clearAndSetSemantics { },
                            gap = 14.dp,
                        )
                        Icon(
                            Icons.AutoMirrored.Rounded.KeyboardArrowRight,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
    }
}

// ── Calendar ────────────────────────────────────────────────────────────────────────────────────

private fun LazyListScope.calendarItems(state: InsightsState, periodName: String, onDay: (LocalDate) -> Unit) {
    item(key = "calendar-hero") { CalendarHero(state, periodName, onDay, Modifier.padding(horizontal = GUTTER)) }
    item(key = "calendar-tiles") { CalendarTiles(state, Modifier.padding(horizontal = GUTTER)) }
    item(key = "calendar-weekdays") { WeekdayPanel(state, periodName, Modifier.padding(horizontal = GUTTER)) }
}

/** The days as a heat grid in the lit panel: how many had spending, then the grid and its key. */
@Composable
private fun CalendarHero(state: InsightsState, periodName: String, onDay: (LocalDate) -> Unit, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val c = state.calendar
    val today = state.today
    val meta = if (state.isCurrentPeriod) "DAY ${state.elapsedDays} OF ${state.period.days}" else Copy.plural(state.period.days, "day").uppercase()
    val supporting = if (state.isCurrentPeriod) {
        "of " + Copy.plural(state.elapsedDays, "day") + " so far had spending"
    } else {
        "of " + Copy.plural(state.period.days, "day") + " had spending"
    }
    val busiest = c.busiestDate
    val caption = if (busiest != null) {
        "Brighter is more spent. Busiest day: " + Dates.day(busiest, today) + " " + money.formatWhole(c.busiestTotal)
    } else {
        "Brighter is more spent. " + nothingSpentLine(state, periodName)
    }
    val chips = buildList {
        if (state.isCurrentPeriod) {
            val spentToday = state.dayTotals.getOrElse(state.elapsedDays - 1) { 0L }
            add(Icons.Rounded.CalendarToday to "Today " + money.formatWhole(spentToday))
            add(Icons.Rounded.Event to Copy.plural(state.daysLeft - 1, "day") + " still ahead")
        } else {
            add(Icons.Rounded.CalendarToday to Copy.plural(state.period.days, "day"))
        }
    }
    HeroPanel(modifier) {
        LensHeroLabel("Spending days", meta)
        Spacer(Modifier.height(14.dp))
        HeroFigure(c.activeDays.toString())
        Text(supporting, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(18.dp))
        HeatGrid(
            period = state.period,
            today = today,
            weekStartsMonday = state.weekStartsMonday,
            heat = state.heat,
            cellDescription = { date, i ->
                val v = state.dayTotals.getOrElse(i) { 0L }
                val day = Dates.day(date, today)
                when {
                    date.isAfter(today) -> "$day, still ahead"
                    v > 0L -> "$day, " + money.formatWhole(v) + " spent"
                    else -> "$day, nothing spent"
                }
            },
            onDay = onDay,
            modifier = Modifier.bleed(min = 8.dp, max = 16.dp, columns = 7, column = 48.dp),
        )
        Spacer(Modifier.height(10.dp))
        Caption(caption)
        Spacer(Modifier.height(16.dp))
        ChipRow(chips)
    }
}

@Composable
private fun CalendarTiles(state: InsightsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val c = state.calendar
    val nothing = if (state.isCurrentPeriod) "Nothing spent yet" else "Nothing spent"
    val topDay = c.topWeekday
    Column(modifier, verticalArrangement = Arrangement.spacedBy(FIGURE_GAP)) {
        TileRow(
            first = { m ->
                StatTile(
                    "Quiet days",
                    c.quietDays.toString(),
                    m,
                    detail = if (c.longestQuietRun > 0) "Longest run " + Copy.plural(c.longestQuietRun, "day") else "None so far",
                )
            },
            second = { m ->
                StatTile(
                    "Per spending day",
                    money.formatWhole(c.perActiveDay),
                    m,
                    detail = if (c.activeDays > 0) "Across " + Copy.plural(c.activeDays, "day") else nothing,
                )
            },
        )
        TileRow(
            first = { m ->
                StatTile(
                    "Busiest weekday",
                    topDay?.let { weekdayName(it) } ?: "None yet",
                    m,
                    detail = if (topDay != null) {
                        money.formatWhole(c.topWeekdayTotal) + " across " + Copy.plural(c.topWeekdayDays, "day")
                    } else nothing,
                )
            },
            second = { m ->
                StatTile(
                    "Weekends",
                    shareText(c.weekendTotal, c.total),
                    m,
                    detail = if (c.total > 0L) money.formatWhole(c.weekendTotal) + " of " + money.formatWhole(c.total) else nothing,
                )
            },
        )
    }
}

/**
 * The week's rhythm: each weekday in week order with its average a day, how many of it have passed
 * and what they took, under a thin bar against the heaviest weekday's average.
 */
@Composable
private fun WeekdayPanel(state: InsightsState, periodName: String, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val weekdays = state.calendar.weekdays
    val top = remember(weekdays) { weekdays.maxOfOrNull { it.average } ?: 0L }
    Panel(modifier) {
        PanelHeader("By weekday", meta = if (top > 0L) "AVERAGE A DAY" else null)
        if (top <= 0L) {
            Spacer(Modifier.height(10.dp))
            EmptyLine(nothingSpentLine(state, periodName))
        } else {
            Spacer(Modifier.height(4.dp))
            weekdays.forEach { w ->
                key(w.day) {
                    val name = weekdayName(w.day)
                    val meta = Copy.plural(w.days, "day") + " · " + money.formatWhole(w.total)
                    val average = money.formatWhole(w.average)
                    Column(
                        Modifier
                            .fillMaxWidth()
                            .semantics(mergeDescendants = true) { contentDescription = "$name, $average on average, $meta" }
                            .heightIn(min = 48.dp)
                            .padding(top = 10.dp, bottom = 4.dp),
                    ) {
                        Column(Modifier.clearAndSetSemantics { }) {
                            EndsRow(
                                start = {
                                    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                        Text(name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                                        Text(meta.uppercase(), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    }
                                },
                                end = { Text(average, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground) },
                                modifier = Modifier.fillMaxWidth(),
                            )
                            Spacer(Modifier.height(8.dp))
                            ThinBar(
                                if (top > 0L) (w.average.toFloat() / top).coerceIn(0f, 1f) else 0f,
                                MaterialTheme.colorScheme.primary,
                                description = "$name, $average on average",
                            )
                        }
                    }
                }
            }
        }
    }
}

/** One day's entries in a sheet, each opening its editor. */
@Composable
private fun DaySheet(date: LocalDate, state: InsightsState, onEntry: (Long) -> Unit, onDismiss: () -> Unit) {
    val money = LocalMoney.current
    val rows = state.entriesByDay[date].orEmpty()
    val index = ChronoUnit.DAYS.between(state.period.start, date).toInt()
    val spent = state.dayTotals.getOrElse(index) { 0L }
    val dayName = Dates.day(date, state.today)
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(Modifier.fillMaxWidth().padding(horizontal = GUTTER).padding(bottom = 24.dp)) {
            Row(
                Modifier.fillMaxWidth().heightIn(min = 40.dp).semantics(mergeDescendants = true) { heading() },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                // The reading drops under the day before the day's name would break mid-word.
                EndsRow(
                    start = {
                        Text(dayName.uppercase(), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onBackground)
                    },
                    end = {
                        Text(
                            (Copy.plural(rows.size, "entry", "entries") + " · " + money.formatWhole(spent) + " out").uppercase(),
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    },
                    modifier = Modifier.weight(1f),
                )
            }
            Spacer(Modifier.height(8.dp))
            if (rows.isEmpty()) {
                EmptyLine(
                    nothingLoggedLine(date, state.today),
                    Modifier.padding(vertical = 8.dp),
                )
            } else {
                LazyColumn(Modifier.fillMaxWidth()) {
                    items(rows, key = { it.id }) { row ->
                        LedgerRow(row, meta = "", onClick = { onEntry(row.id) })
                    }
                }
            }
        }
    }
}

// ── Worth ───────────────────────────────────────────────────────────────────────────────────────

private fun LazyListScope.worthItems(state: InsightsState, periodName: String) {
    item(key = "worth-hero") { WorthHero(state, periodName, Modifier.padding(horizontal = GUTTER)) }
    item(key = "worth-tiles") { WorthTiles(state, Modifier.padding(horizontal = GUTTER)) }
    if (state.worth.accounts.isNotEmpty()) {
        item(key = "worth-accounts") { AccountsPanel(state, Modifier.padding(horizontal = GUTTER)) }
        item(key = "worth-months") { WorthListPanel(state, Modifier.padding(horizontal = GUTTER)) }
    }
}

/**
 * What the open accounts add up to at the period's close, how far it moved over the period, and
 * the close of the six periods ending at it as bars from zero (a debt's bar in the error ink).
 */
@Composable
private fun WorthHero(state: InsightsState, periodName: String, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val w = state.worth
    val labels = remember(w.points) { w.points.map { Dates.monthShort(it.period.start) } }
    val description = remember(w.points, money, state.today) {
        if (w.points.isEmpty()) "No months to chart"
        else "Net worth by month: " + w.points.joinToString("; ") { Dates.period(it.period, state.today) + " " + money.formatWhole(it.worth) }
    }
    val moved = when {
        w.change > 0L -> "up " + money.formatWhole(w.change) + " over $periodName"
        w.change < 0L -> "down " + money.formatWhole(-w.change) + " over $periodName"
        else -> "no change over $periodName"
    }
    val chips = buildList {
        state.savingsRate?.let { rate ->
            add(Icons.Rounded.Savings to if (rate >= 0) "Kept $rate% of income" else "Spent ${-rate}% more than came in")
        }
        if (w.invested != 0L) add(Icons.AutoMirrored.Rounded.ShowChart to "Invested " + signedWhole(money, w.invested))
    }
    HeroPanel(modifier) {
        LensHeroLabel("Net worth", if (state.isCurrentPeriod) "TODAY" else "AT MONTH END")
        Spacer(Modifier.height(14.dp))
        HeroFigure(money.formatWhole(w.worth))
        Text(
            if (w.accounts.isEmpty()) "across no accounts yet" else moved,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (w.accounts.isEmpty()) {
            Spacer(Modifier.height(14.dp))
            EmptyLine("Add an account to follow what you hold and what you owe")
            return@HeroPanel
        }
        Spacer(Modifier.height(18.dp))
        WorthBars(w.points, labels, description)
        if (chips.isNotEmpty()) {
            Spacer(Modifier.height(16.dp))
            ChipRow(chips)
        }
    }
}

/** Six closes as bars from zero; the period picked lit, the ones before it quieter. */
@Composable
private fun WorthBars(points: List<WorthPoint>, labels: List<String>, description: String, modifier: Modifier = Modifier) {
    val lit = MaterialTheme.colorScheme.primary
    val quiet = MaterialTheme.colorScheme.secondary.copy(alpha = 0.55f)
    val owed = MaterialTheme.colorScheme.error
    val top = points.maxOfOrNull { kotlin.math.abs(it.worth) }?.coerceAtLeast(1L) ?: 1L
    Row(
        modifier.fillMaxWidth().height(136.dp).clearAndSetSemantics { contentDescription = description },
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        points.forEachIndexed { i, p ->
            val fraction = (kotlin.math.abs(p.worth).toFloat() / top).coerceIn(0.03f, 1f)
            val color = when {
                p.worth < 0L -> owed
                i == points.lastIndex -> lit
                else -> quiet
            }
            Column(Modifier.weight(1f).fillMaxHeight(), horizontalAlignment = Alignment.CenterHorizontally) {
                Box(Modifier.fillMaxWidth().weight(1f), contentAlignment = Alignment.BottomCenter) {
                    Box(
                        Modifier
                            .fillMaxWidth(0.72f)
                            .fillMaxHeight(fraction)
                            .clip(RoundedCornerShape(6.dp))
                            .background(color),
                    )
                }
                Spacer(Modifier.height(6.dp))
                Text(
                    labels.getOrElse(i) { "" },
                    style = MaterialTheme.typography.labelSmall,
                    color = if (i == points.lastIndex) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

/** What is held against what is owed, then what the period kept and what it put to work. */
@Composable
private fun WorthTiles(state: InsightsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val w = state.worth
    val held = w.accounts.count { it.balance > 0L }
    val owing = w.accounts.count { it.balance < 0L }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(FIGURE_GAP)) {
        TileRow(
            first = { m ->
                StatTile(
                    "Held",
                    money.formatWhole(w.assets),
                    m,
                    detail = if (held == 0) "Nothing in credit" else "In " + Copy.plural(held, "account"),
                )
            },
            second = { m ->
                StatTile(
                    "Owed",
                    money.formatWhole(w.debts),
                    m,
                    detail = if (owing == 0) "Nothing owed" else "On " + Copy.plural(owing, "account"),
                )
            },
        )
        TileRow(
            first = { m ->
                val rate = state.savingsRate
                StatTile(
                    "Kept",
                    if (rate == null) "None" else "$rate%",
                    m,
                    detail = if (rate == null) "No income logged" else "Of " + money.formatWhole(state.income) + " income",
                )
            },
            second = { m ->
                StatTile(
                    "Invested",
                    signedWhole(money, w.invested),
                    m,
                    detail = if (w.investmentAccounts == 0) "No investment account yet"
                    else "Holding " + money.formatWhole(w.investments),
                )
            },
        )
    }
}

/** Every open account at the period's close: the ones in credit by their share of what is held, then the debts. */
@Composable
private fun AccountsPanel(state: InsightsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val w = state.worth
    Panel(modifier) {
        PanelHeader("Accounts", meta = w.accounts.size.toString())
        Spacer(Modifier.height(4.dp))
        w.accounts.forEach { a ->
            key(a.id) {
                val owed = a.balance < 0L
                val whole = if (owed) w.debts else w.assets
                val share = shareText(kotlin.math.abs(a.balance), whole)
                val valued = a.valuedOn?.let { " · valued " + Dates.short(it, state.today) }.orEmpty()
                val meta = typeLabel(a.type) + valued
                val amount = money.formatWhole(a.balance)
                Column(
                    Modifier
                        .fillMaxWidth()
                        .semantics(mergeDescendants = true) {
                            contentDescription = "${a.name}, $amount, $meta, $share of " + if (owed) "what is owed" else "what is held"
                        }
                        .heightIn(min = 64.dp)
                        .padding(top = 10.dp, bottom = 6.dp),
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                        GlyphBadge(CategoryIcons.account(a.type), size = 36.dp)
                        EndsRow(
                            start = {
                                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                    Text(a.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                                    Text(meta, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            },
                            end = {
                                Text(
                                    amount,
                                    style = MaterialTheme.typography.titleSmall,
                                    color = if (owed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onBackground,
                                    textAlign = TextAlign.End,
                                )
                            },
                            modifier = Modifier.weight(1f).clearAndSetSemantics { },
                        )
                    }
                    Spacer(Modifier.height(10.dp))
                    ThinBar(
                        if (whole > 0L) kotlin.math.abs(a.balance).toFloat() / whole else 0f,
                        if (owed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.tertiary,
                        description = "$share of " + if (owed) "what is owed" else "what is held",
                    )
                }
            }
        }
    }
}

/** The six closes as rows, newest first, each with how far it moved from the one before. */
@Composable
private fun WorthListPanel(state: InsightsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val points = state.worth.points
    Panel(modifier) {
        PanelHeader("Month by month", meta = "NEWEST FIRST")
        Spacer(Modifier.height(4.dp))
        points.indices.reversed().forEach { i ->
            val p = points[i]
            key(p.offset) {
                val name = Dates.period(p.period, state.today) + if (state.today in p.period) " so far" else ""
                val change = points.getOrNull(i - 1)?.let { p.worth - it.worth }
                val meta = when {
                    change == null -> "Start of the chart"
                    change > 0L -> "Up " + money.formatWhole(change)
                    change < 0L -> "Down " + money.formatWhole(-change)
                    else -> "No change"
                }
                val amount = money.formatWhole(p.worth)
                Row(
                    Modifier
                        .fillMaxWidth()
                        .semantics(mergeDescendants = true) { contentDescription = "$name, $amount, $meta" }
                        .heightIn(min = 56.dp)
                        .padding(vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    EndsRow(
                        start = {
                            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                Text(name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                                Text(
                                    meta,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = if ((change ?: 0L) < 0L) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        },
                        end = {
                            Text(amount, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground, textAlign = TextAlign.End)
                        },
                        modifier = Modifier.weight(1f).clearAndSetSemantics { },
                    )
                }
            }
        }
    }
}
