package com.tally.app.ui.insights

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.EntryRow
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.kit.AccountBadge
import com.tally.app.ui.kit.Balance
import com.tally.app.ui.kit.CategoryBadge
import com.tally.app.ui.kit.Dot
import com.tally.app.ui.kit.Empty
import com.tally.app.ui.kit.HeatGrid
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.Meter
import com.tally.app.ui.kit.Panel
import com.tally.app.ui.kit.Segmented
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.SpacePage
import com.tally.app.ui.kit.SpendLine
import com.tally.app.ui.kit.Sparkline
import com.tally.app.ui.kit.StackBar
import com.tally.app.ui.kit.Stat
import com.tally.app.ui.kit.StatGrid
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.TrendBars
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.Tally
import com.tally.app.ui.theme.categoryColor
import com.tally.core.Copy
import java.time.temporal.ChronoUnit

@Composable
fun InsightsRoute(nav: AppNav, delete: (Long) -> Unit) {
    val vm: InsightsViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { vm.onResume() }
    InsightsScreen(
        s,
        previous = vm::previousPeriod,
        next = vm::nextPeriod,
        showPeriod = vm::showPeriod,
        category = { id -> if (id != null) nav.transactions(categoryId = id) },
        entry = { nav.entry(id = it) },
        delete = delete,
        account = { nav.transactions(accountId = it) },
    )
}

/** Where the month's money went and how it moved: the month, the trend, the calendar, and what you are worth once an account keeps a balance. */
@Composable
fun InsightsScreen(
    s: InsightsState,
    previous: () -> Unit,
    next: () -> Unit,
    showPeriod: (Int) -> Unit,
    category: (Long?) -> Unit,
    entry: (Long) -> Unit,
    delete: (Long) -> Unit,
    account: (Long) -> Unit,
) {
    var lens by rememberSaveable { mutableStateOf(InsightsLens.MONTH) }
    val fmt = LocalMoney.current
    SpacePage(
        Dates.period(s.period, s.today),
        context = if (s.loaded) fmt.formatWhole(s.spent) + " out · " + fmt.formatWhole(s.income) + " in" + (s.savingsRate?.let { " · kept $it%" } ?: "") else null,
        actions = {
            IconKey("chevron-left", previous, label = "Earlier month")
            IconKey("chevron-right", next, enabled = s.isCurrentPeriod.not(), label = "Later month")
        },
    ) {
        // Worth reads only when some account keeps a balance: Tally tracks spending, and a worth
        // built from spending alone would read as a debt that is not there.
        val lenses = InsightsLens.entries.filter { it != InsightsLens.WORTH || s.worth.tracked }
        val shown = if (lens in lenses) lens else InsightsLens.MONTH
        Segmented(lenses.map { it to it.label }, shown, { lens = it }, Modifier.fillMaxWidth(), fill = true)
        if (!s.loaded) return@SpacePage
        when (shown) {
            InsightsLens.MONTH -> MonthLens(s, category, entry, delete)
            InsightsLens.TREND -> TrendLens(s, showPeriod)
            InsightsLens.CALENDAR -> CalendarLens(s, entry, delete)
            InsightsLens.WORTH -> WorthLens(s, account)
        }
    }
}

@Composable
private fun MonthLens(s: InsightsState, category: (Long?) -> Unit, entry: (Long) -> Unit, delete: (Long) -> Unit) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    var picked by rememberSaveable(s.period.start) { mutableStateOf<Int?>(null) }
    val day = picked?.takeIf { it < s.cumulative.size }
    Panel("The month", glyph = "trend", subtitle = s.budget?.let { "Against " + fmt.formatWhole(it) + ", the dashed line" } ?: "Drag along the line to read a day") {
        Column(Modifier.padding(4.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            val shown = day ?: (s.cumulative.size - 1)
            if (shown >= 0 && s.cumulative.isNotEmpty()) {
                val date = s.period.start.plusDays(shown.toLong())
                val pace = s.budget?.let { paceAt(it, shown, s.period.days) }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    T(Dates.day(date, s.today), Modifier.weight(1f), Tally.type.caption, c.ink3)
                    T(fmt.formatWhole(s.cumulative[shown]) + (pace?.let { " · pace " + fmt.formatWhole(it) } ?: ""), style = Tally.type.mono, color = if (pace != null && s.cumulative[shown] > pace) c.waiting else c.ink2)
                }
            }
            SpendLine(
                s.cumulative,
                s.period.days,
                "Spent by day this month, " + fmt.formatWhole(s.spent) + " so far" + (s.budget?.let { " of " + fmt.formatWhole(it) } ?: ""),
                budget = s.budget,
                picked = day,
                onScrub = { picked = it },
            )
        }
    }
    StatGrid(
        listOf(
            { m -> Stat("A day", fmt.formatWhole(s.perDay), m, glyph = "calendar", sub = "over " + Copy.plural(s.elapsedDays, "day")) },
            { m -> Stat("Projected", s.projected?.let { fmt.formatWhole(it) } ?: "–", m, glyph = "trend", sub = if (s.projected != null) "at this pace" else "once the month is under way") },
            { m -> Stat("Vs last month", fmt.formatWhole(s.lastPeriodSameDay), m, glyph = "history", sub = Copy.versusLastLine(s.spent, s.lastPeriodSameDay, fmt)) },
            { m -> Stat("Kept", s.savingsRate?.let { "$it%" } ?: "–", m, glyph = "wallet", sub = if (s.income > 0) "of what came in" else "nothing came in yet") },
        ),
    )
    if (s.categories.isEmpty()) {
        Slab { Empty("pie", "Nothing spent in this month", null) }
        return
    }
    Panel("Where it went", glyph = "pie", subtitle = Copy.plural(s.categoryCount, "category", "categories")) {
        StackBar(s.categories.map { categoryColor(it.color) to it.total }, Modifier.padding(horizontal = 4.dp, vertical = 6.dp))
        s.categories.forEach { slice ->
            ListRow(onClick = { if (!slice.isFolded) category(slice.categoryId) }, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
                CategoryBadge(slice.icon, slice.color)
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        T(slice.name, Modifier.weight(1f), Tally.type.ui, c.ink, maxLines = 1)
                        T(fmt.formatWhole(slice.total), style = Tally.type.amount, color = c.ink2)
                    }
                    Meter(if (s.spent > 0) slice.total.toFloat() / s.spent else 0f, color = categoryColor(slice.color), height = 3.dp)
                    T(Copy.plural(slice.count, "entry", "entries") + " · " + percentOf(slice.total, s.spent) + "%", style = Tally.type.caption, color = c.ink3)
                }
            }
        }
    }
    if (s.incomeSources.isNotEmpty()) {
        Panel("Where it came from", glyph = "money-in") {
            s.incomeSources.forEach { slice ->
                ListRow(onClick = { if (!slice.isFolded) category(slice.categoryId) }, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
                    CategoryBadge(slice.icon, slice.color)
                    T(slice.name, Modifier.weight(1f), Tally.type.ui, c.ink, maxLines = 1)
                    T("+" + fmt.formatWhole(slice.total), style = Tally.type.amount, color = c.liveText)
                }
            }
        }
    }
    if (s.payees.isNotEmpty()) {
        Panel("Top payees", glyph = "receipt") {
            s.payees.forEach { p ->
                Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        T(p.note, style = Tally.type.ui, color = c.ink, maxLines = 1)
                        T(Copy.plural(p.count, "time"), style = Tally.type.caption, color = c.ink3)
                    }
                    T(fmt.formatWhole(p.total), style = Tally.type.amount, color = c.ink2)
                }
            }
        }
    }
    if (s.largest.isNotEmpty()) {
        Panel("Largest", glyph = "bars", subtitle = fmt.formatWhole(s.largestTotal) + " together") {
            s.largest.forEach { row -> EntryRow(row, onClick = { entry(row.id) }, onLongClick = { delete(row.id) }, today = s.today) }
        }
    }
}

@Composable
private fun TrendLens(s: InsightsState, showPeriod: (Int) -> Unit) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val t = s.trendSummary
    val labels = s.trend.map { Dates.monthShort(it.period.start) }
    Panel("Six months", glyph = "bars", subtitle = "Out in ink, in in green; tap a month to read it") {
        TrendBars(
            s.trend.map { it.spent },
            s.trend.map { it.income },
            labels,
            "Spent and earned over the last " + Copy.plural(s.trend.size, "month"),
            Modifier.padding(4.dp),
            highlight = s.trend.indexOfFirst { it.offset == s.offset }.coerceAtLeast(0),
            onPick = { i -> s.trend.getOrNull(i)?.let { showPeriod(it.offset) } },
        )
    }
    StatGrid(
        listOf(
            { m -> Stat("Spent a month", fmt.formatWhole(t.averageSpent), m, glyph = "money-out", sub = "average of " + Copy.plural(t.months, "month")) },
            { m -> Stat("Earned a month", fmt.formatWhole(t.averageIncome), m, glyph = "money-in", sub = "average of " + Copy.plural(t.months, "month")) },
            { m -> Stat("Net a month", fmt.formatWhole(t.averageNet), m, glyph = "wallet", figureColor = if (t.averageNet < 0) c.heldText else c.ink) },
            { m -> Stat("Over budget", Copy.plural(t.overBudget, "month"), m, glyph = "flag", sub = "of the last " + s.trend.size) },
        ),
    )
    Panel("By month", glyph = "list") {
        s.trend.asReversed().forEach { p ->
            ListRow(onClick = { showPeriod(p.offset) }, selected = p.offset == s.offset, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
                T(Dates.period(p.period, s.today) + if (p.running) " (so far)" else "", Modifier.weight(1f), Tally.type.ui, c.ink, maxLines = 1)
                T(fmt.formatWhole(p.spent), style = Tally.type.amount, color = c.ink2)
                T((if (p.net >= 0) "+" else "−") + fmt.formatWhole(kotlin.math.abs(p.net)), style = Tally.type.mono, color = if (p.net >= 0) c.liveText else c.heldText)
            }
        }
    }
}

@Composable
private fun CalendarLens(s: InsightsState, entry: (Long) -> Unit, delete: (Long) -> Unit) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val cal = s.calendar
    var picked by rememberSaveable(s.period.start) { mutableStateOf<Int?>(null) }
    val first = if (s.weekStartsMonday) java.time.DayOfWeek.MONDAY else java.time.DayOfWeek.SUNDAY
    val lead = ((s.period.start.dayOfWeek.value - first.value) + 7) % 7
    val weekdays = List(7) { Dates.weekdayInitial(first.plus(it.toLong())) }
    Panel("Days", glyph = "calendar", subtitle = "Brighter is more spent; tap a day for its entries") {
        HeatGrid(
            s.heat,
            lead,
            s.elapsedDays,
            weekdays,
            Copy.plural(cal.activeDays, "day") + " with spending of " + cal.daysSoFar,
            Modifier.padding(4.dp),
            picked = picked,
            onPick = { picked = it },
        )
    }
    picked?.let { i ->
        val date = s.period.start.plusDays(i.toLong())
        val rows = s.entriesByDay[date].orEmpty()
        Panel(Dates.day(date, s.today), glyph = "list", subtitle = fmt.formatWhole(s.dayTotals.getOrElse(i) { 0L }) + " spent") {
            if (rows.isEmpty()) T(nothingLoggedLine(date, s.today), Modifier.padding(4.dp), Tally.type.caption, c.ink3)
            rows.forEach { row -> EntryRow(row, onClick = { entry(row.id) }, onLongClick = { delete(row.id) }) }
        }
    }
    StatGrid(
        listOf(
            { m -> Stat("Days with spending", cal.activeDays.toString(), m, glyph = "calendar", sub = "of " + cal.daysSoFar + " so far") },
            { m -> Stat("Longest quiet run", Copy.plural(cal.longestQuietRun, "day"), m, glyph = "pause") },
            { m -> Stat("Busiest day", cal.busiestDate?.let { Dates.short(it, s.today) } ?: "–", m, glyph = "flag", sub = if (cal.busiestTotal > 0) fmt.formatWhole(cal.busiestTotal) else null) },
            { m -> Stat("Weekends", fmt.formatWhole(cal.weekendTotal), m, glyph = "sparkle", sub = cal.topWeekday?.let { "Most on " + Dates.weekday(it) + "s" }) },
        ),
    )
    if (cal.weekdays.isNotEmpty()) {
        Panel("By weekday", glyph = "bars", subtitle = "What a day of each kind averages") {
            val top = (cal.weekdays.maxOfOrNull { it.average } ?: 0L).coerceAtLeast(1L)
            cal.weekdays.forEach { w ->
                Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 5.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    T(Dates.weekday(w.day).take(3), style = Tally.type.mono, color = c.ink3)
                    Meter(w.average.toFloat() / top, Modifier.weight(1f), color = if (w.day == cal.topWeekday) c.ink else c.ink3)
                    T(fmt.formatWhole(w.average), style = Tally.type.mono, color = c.ink2)
                }
            }
        }
    }
}

@Composable
private fun WorthLens(s: InsightsState, account: (Long) -> Unit) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val w = s.worth
    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(16.dp), spacing = 8.dp) {
        T("Worth at the end of " + Dates.period(s.period, s.today), style = Tally.type.caption, color = c.ink3)
        T(fmt.formatWhole(w.worth), style = Tally.type.hero, color = if (w.worth < 0) c.heldText else c.ink, maxLines = 1)
        T(
            (if (w.change >= 0) "Up " else "Down ") + fmt.formatWhole(kotlin.math.abs(w.change)) + " over the month",
            style = Tally.type.caption,
            color = if (w.change >= 0) c.liveText else c.heldText,
        )
        Sparkline(w.points.map { it.worth }, color = if (w.change >= 0) c.ink else c.heldText)
    }
    StatGrid(
        listOf(
            { m -> Stat("Held", fmt.formatWhole(w.assets), m, glyph = "wallet") },
            { m -> Stat("Owed", fmt.formatWhole(w.debts), m, glyph = "card", figureColor = if (w.debts > 0) c.heldText else c.ink) },
            { m -> Stat("Invested this month", fmt.formatWhole(w.invested), m, glyph = "trend", sub = Copy.plural(w.investmentAccounts, "investment account")) },
            { m -> Stat("In investments", fmt.formatWhole(w.investments), m, glyph = "coins") },
        ),
    )
    if (w.accounts.isEmpty()) return
    Panel("Accounts", glyph = "bank") {
        w.accounts.forEach { a ->
            ListRow(onClick = { account(a.id) }, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
                AccountBadge(a.type)
                Column(Modifier.weight(1f)) {
                    T(a.name, style = Tally.type.ui, color = c.ink, maxLines = 1)
                    a.valuedOn?.let { T("Valued " + Dates.short(it, s.today), style = Tally.type.caption, color = c.ink3) }
                }
                Balance(a.balance)
            }
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Dot(c.ink3)
        T("Balances are read from your entries and the values you recorded.", style = Tally.type.caption, color = c.ink3)
    }
}

private fun percentOf(part: Long, whole: Long): Int = if (whole <= 0L) 0 else Math.round(part * 100.0 / whole).toInt()

/** Days between two dates, for a spoken reading. */
internal fun daysBetween(a: java.time.LocalDate, b: java.time.LocalDate): Long = ChronoUnit.DAYS.between(a, b)
