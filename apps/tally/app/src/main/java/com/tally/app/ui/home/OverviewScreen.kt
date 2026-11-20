package com.tally.app.ui.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.clickable
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.sp
import com.tally.app.ui.kit.DatedBillRow
import com.tally.app.ui.kit.Donut
import com.tally.app.ui.kit.Dot
import com.tally.app.ui.kit.Gauge
import com.tally.app.ui.kit.Meter
import com.tally.app.ui.kit.Postcard
import com.tally.app.ui.kit.Radii
import com.tally.app.ui.kit.Scene
import com.tally.app.ui.kit.StatePill
import com.tally.app.ui.kit.TintTile
import com.tally.app.ui.kit.dateHue
import com.tally.app.ui.kit.sceneFor
import com.tally.app.ui.plan.GoalLine
import com.tally.app.ui.plan.GoalStatus
import com.tally.app.ui.plan.goalGlyph
import com.tally.app.ui.plan.goalKindLabel
import com.tally.app.ui.plan.goalReading
import com.tally.app.ui.plan.goalRowLine
import com.tally.app.ui.theme.Hue
import com.tally.core.GoalKind
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.EntryRow
import com.tally.app.ui.common.LineTone
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.kit.Empty
import com.tally.app.ui.kit.Eyebrow
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.PaceMeter
import com.tally.app.ui.kit.Panel
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.SpacePage
import com.tally.app.ui.kit.StatGrid
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.WeekBars
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.plan.BudgetRow
import com.tally.app.ui.plan.limitReached
import com.tally.app.ui.theme.Tally
import com.tally.app.ui.theme.categoryColor
import com.tally.core.Copy
import com.tally.core.TxType

/** What Overview can ask for, as plain lambdas, so the page is drawn from state alone. */
data class OverviewActions(
    val entry: (Long) -> Unit = {},
    val deleteEntry: (Long) -> Unit = {},
    val entries: () -> Unit = {},
    val budgets: () -> Unit = {},
    val setBudget: () -> Unit = {},
    val budget: (Long) -> Unit = {},
    val bills: () -> Unit = {},
    val bill: (Long) -> Unit = {},
    val goals: () -> Unit = {},
    val goal: (Long) -> Unit = {},
    val accounts: () -> Unit = {},
    val account: (Long) -> Unit = {},
    val addAccount: () -> Unit = {},
    val add: () -> Unit = {},
    val import: () -> Unit = {},
    val insights: () -> Unit = {},
    /** A category's entries. */
    val category: (Long) -> Unit = {},
)

@Composable
fun OverviewRoute(nav: AppNav, deleteEntry: (Long) -> Unit) {
    val vm: HomeViewModel = hiltViewModel()
    val state by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { vm.onResume() }
    OverviewScreen(
        state,
        OverviewActions(
            entry = { nav.entry(id = it) },
            deleteEntry = deleteEntry,
            entries = nav::entries,
            budgets = nav::budgets,
            setBudget = { nav.budgetEdit(0) },
            budget = { nav.budgetEdit(it) },
            bills = nav::bills,
            bill = { nav.billEdit(it) },
            goals = nav::goals,
            goal = { nav.goal(it) },
            accounts = nav::accounts,
            account = { nav.transactions(accountId = it) },
            addAccount = { nav.accountEdit() },
            add = { nav.entry() },
            import = { nav.import() },
            insights = nav::insights,
            category = { nav.transactions(categoryId = it) },
        ),
    )
}

/**
 * The month at a glance, direction C: the gauge with what is left of the limit (or that it is
 * reached) in its middle, today and each day on tinted tiles, the week, the budgets furthest over
 * pace, where the money went as a donut, the bills coming up by date, recent entries and goals as
 * postcards. It reads spending, not what the accounts hold.
 */
@Composable
fun OverviewScreen(s: HomeState, a: OverviewActions) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val r = s.reading
    val context = Copy.plural(s.daysLeft, "day") + " left · resets " + Dates.short(s.period.endExclusive, s.today)
    SpacePage(Dates.period(s.period, s.today), context = context) {
        if (!s.loaded) return@SpacePage
        if (!s.hasEntries && s.accounts.isEmpty()) {
            Postcard(Scene.HILLS, sceneHeight = 120.dp) {
                Empty(
                    "wallet",
                    "Nothing logged yet",
                    "Add an account and log what you spend; Tally keeps the month's pace from there. Or import a bank statement.",
                    Modifier.padding(horizontal = 0.dp),
                ) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Key("Add an account", a.addAccount, kind = KeyKind.Primary, glyph = "bank")
                        Key("Import", a.import, glyph = "download")
                    }
                }
            }
            return@SpacePage
        }

        // The reading: what is left of the month's limit, or that the limit is reached; never a negative.
        if (r != null) {
            val reached = limitReached(r)
            Column(
                Modifier.fillMaxWidth().clip(Radii.card).clickable(role = Role.Button, onClick = a.budgets).padding(bottom = 4.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Gauge(r.spentFraction, Modifier.widthIn(max = 330.dp), pace = r.paceFraction) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        T(
                            if (reached) "Limit reached" else "Left this month",
                            style = Tally.type.uiMedium,
                            color = if (reached) c.heldText else c.ink2,
                            maxLines = 1,
                        )
                        T(
                            fmt.formatWhole(if (reached) r.spent else r.remaining),
                            style = Tally.type.hero,
                            color = if (reached) c.heldText else c.ink,
                            maxLines = 1,
                        )
                    }
                }
                T(
                    Copy.ofBudget(r.spent, r.budget, fmt) + " spent · " +
                        (if (reached) Copy.plural(r.daysLeft, "day") + " to go" else Copy.marginLine(r, fmt)),
                    style = Tally.type.caption,
                    color = c.ink2,
                    maxLines = 2,
                    align = TextAlign.Center,
                )
                val line = heroLine(r, fmt)
                StatePill(line.text, line.tone, glyph = if (line.tone == LineTone.QUIET) "check" else null)
            }
        } else {
            Slab(Modifier.fillMaxWidth(), padding = PaddingValues(18.dp), spacing = 10.dp, onClick = a.budgets) {
                Eyebrow("Spent this month")
                T(fmt.formatWhole(s.spent), style = Tally.type.hero, color = c.ink, maxLines = 1)
                T("Set a monthly limit and Tally reads the month as a pace: how much is left, and what each day can take.", style = Tally.type.caption, color = c.ink2)
                Key("Set a monthly budget", a.setBudget, kind = KeyKind.Primary, glyph = "pie", compact = true)
            }
        }

        val tiles: List<@Composable (Modifier) -> Unit> =
            listOf(
                { m ->
                    val today = todayLine(s.spentToday, r, fmt)
                    TintTile("Today", fmt.formatWhole(s.spentToday), Hue.mint, m, sub = today.text, glyph = "receipt", onClick = a.entries)
                },
                { m ->
                    TintTile(
                        "Each day can take",
                        if (r != null) fmt.formatWhole(r.dailyAllowance) else "–",
                        Hue.coral,
                        m,
                        glyph = "calendar",
                        sub = when {
                            r == null -> "with a monthly budget"
                            limitReached(r) -> "Nothing left until " + Dates.short(s.period.endExclusive, s.today)
                            else -> "for " + Copy.plural(r.daysLeft, "day")
                        },
                    )
                },
                { m -> TintTile("Money out", fmt.formatWhole(s.spent), Hue.sky, m, glyph = "money-out", sub = Copy.versusLastLine(s.spent, s.lastPeriodSameDay, fmt)) },
                { m -> TintTile("Money in", fmt.formatWhole(s.income), Hue.violet, m, glyph = "money-in", sub = versus(s.income, s.lastPeriodIncomeSameDay, fmt, "in")) },
            )
        // At a large font two tiles to a row would cut their words: one to a row instead.
        if (LocalDensity.current.fontScale >= 1.5f) {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) { tiles.forEach { it(Modifier.fillMaxWidth()) } }
        } else {
            StatGrid(tiles)
        }

        Panel("This week", subtitle = fmt.formatWhole(s.weekSpent) + " spent") {
            val labels = List(7) { Dates.weekdayInitial(s.weekStart.plusDays(it.toLong()).dayOfWeek) }
            val description = weekDescription(s.weekStart, s.week, s.todayInWeek, s.weekSpentBeforePeriod, s.period.start) { fmt.formatWhole(it) }
            WeekBars(s.week, s.todayInWeek, labels, description, Modifier.padding4(), daily = s.dailyBudget)
        }

        if (s.envelopes.isNotEmpty()) {
            Panel("Budgets", action = "All", onAction = a.budgets) {
                s.envelopes.take(4).forEach { e -> BudgetRow(e.name, e.icon, e.color, e.reading, onClick = { a.budget(e.categoryId) }) }
            }
        }

        if (s.categories.isNotEmpty()) {
            Panel("Where it went", action = "Insights", onAction = a.insights) {
                Row(Modifier.fillMaxWidth().padding(top = 4.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    Donut(s.categories.map { categoryColor(it.color) to it.total }, size = 118.dp, thickness = 20.dp) {
                        T(fmt.formatWhole(s.spent), style = Tally.type.title.copy(fontSize = 15.sp), color = c.ink, maxLines = 1)
                    }
                    Column(Modifier.weight(1f)) {
                        s.categories.forEach { share ->
                            ListRow(onClick = share.categoryId?.let { id -> { a.category(id) } }, padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                                Dot(categoryColor(share.color), 10.dp)
                                Column(Modifier.weight(1f)) {
                                    T(share.name, style = Tally.type.ui, color = c.ink, maxLines = 1, weight = FontWeight.Medium)
                                    T(Copy.plural(share.count, "entry", "entries") + " · " + percentOf(share.total, s.spent) + "%", style = Tally.type.caption, color = c.ink3, maxLines = 1)
                                }
                                T(fmt.formatWhole(share.total), style = Tally.type.amount, color = c.ink, weight = FontWeight.SemiBold)
                            }
                        }
                    }
                }
            }
        }

        if (s.upcoming.isNotEmpty()) {
            Panel("Coming up", action = "Bills", onAction = a.bills) {
                s.upcoming.forEachIndexed { i, b ->
                    DatedBillRow(
                        b.name, b.date,
                        line = Copy.dueLine(b.daysUntil) + " · " + b.accountName + if (b.autoPost) "" else " · reminder",
                        type = b.type,
                        amount = b.amount,
                        hue = dateHue(i),
                        onClick = { a.bill(b.id) },
                        due = b.daysUntil,
                        autoPost = b.autoPost,
                    )
                }
            }
        }

        Panel("Recent", action = "All", onAction = a.entries) {
            if (s.recent.isEmpty()) {
                T("Nothing logged this month yet.", Modifier.padding4(), Tally.type.caption, c.ink3)
            }
            s.recent.forEach { row -> EntryRow(row, onClick = { a.entry(row.id) }, onLongClick = { a.deleteEntry(row.id) }, today = s.today) }
        }

        if (s.goals.isNotEmpty()) {
            Row(Modifier.fillMaxWidth().padding(start = 4.dp, top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                T("Goals", Modifier.weight(1f), Tally.type.section, c.ink)
                T("All", Modifier.clip(Radii.key).clickable(role = Role.Button, onClick = a.goals).padding(horizontal = 10.dp, vertical = 8.dp), Tally.type.uiMedium, c.signal, weight = FontWeight.SemiBold)
            }
            s.goals.forEach { g -> GoalCard(g, s.today, onClick = { a.goal(g.id) }) }
        }
    }
}

/** A goal as a postcard: a scene from its name, its reading, its meter and the line it needs. */
@Composable
internal fun GoalCard(g: GoalLine, today: java.time.LocalDate, onClick: () -> Unit) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    Postcard(sceneFor(g.name, invest = g.kind == GoalKind.INVEST), tag = if (g.reached) "Reached" else goalKindLabel(g.kind), tagGlyph = goalGlyph(g.kind), onClick = onClick) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            T(g.name, Modifier.weight(1f), Tally.type.title, c.ink, maxLines = 1)
            T(goalReading(g, fmt), style = Tally.type.amount, color = c.ink2, maxLines = 1, weight = FontWeight.SemiBold)
        }
        if (g.paceFraction != null) PaceMeter(g.fraction, g.paceFraction) else Meter(g.fraction, color = if (g.reached) c.live else c.signal)
        T(
            goalRowLine(g, fmt, today),
            style = Tally.type.caption,
            color = if (g.status == GoalStatus.BEHIND) c.waitingText else if (g.reached) c.liveText else c.ink3,
            maxLines = 2,
        )
    }
}

/** A whole percent of [total], for a share line; zero of nothing is zero. */
private fun percentOf(part: Long, total: Long): Int = if (total <= 0L) 0 else Math.round(part * 100.0 / total).toInt()

private fun Modifier.padding4(): Modifier = this.then(Modifier.fillMaxWidth())

/** Money in against last month by the same day, in the voice of [Copy.versusLastLine]. */
private fun versus(now: Long, last: Long, fmt: com.tally.core.MoneyFormatter, word: String): String {
    if (last <= 0L) return if (now == 0L) "Nothing $word yet" else "Nothing by this day last month"
    val diff = now - last
    return when {
        diff == 0L -> "Level with last month"
        diff > 0 -> fmt.formatWhole(diff) + " more than last month"
        else -> fmt.formatWhole(-diff) + " less than last month"
    }
}

/** Entries of a type, for a summary line. */
internal fun typeWord(type: TxType): String = when (type) {
    TxType.EXPENSE -> "spent"
    TxType.INCOME -> "in"
    TxType.TRANSFER -> "moved"
}
