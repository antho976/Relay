package com.tally.app.ui.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.TrendingUp
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.AccountBadge
import com.tally.app.ui.common.Aside
import com.tally.app.ui.common.CategoryBadge
import com.tally.app.ui.common.ChromeButton
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.DayBars
import com.tally.app.ui.common.EndsRow
import com.tally.app.ui.common.FIGURE_GAP
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LedgerRow
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PaceMeter
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.Panel
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.READING_WIDTH
import com.tally.app.ui.common.ROW_FOCUS_OUTSET
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.StatTile
import com.tally.app.ui.common.TextAction
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.common.TransferBadge
import com.tally.app.ui.common.bounceClick
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.nav.FAB_CLEARANCE
import com.tally.app.ui.nav.HubTab
import com.tally.app.ui.plan.GoalRow
import com.tally.core.Copy
import com.tally.core.PaceStatus
import com.tally.core.TxType
import java.time.LocalDate

@Composable
fun HomeTab(nav: AppNav, openTab: (HubTab) -> Unit, viewModel: HomeViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    LifecycleResumeEffect(viewModel) {
        viewModel.onResume()
        onPauseOrDispose { }
    }
    HomeScreen(
        state = state,
        actions = HomeActions(
            settings = nav::settings,
            setBudget = { nav.budgetEdit(0) },
            openPlan = { openTab(HubTab.PLAN) },
            openHistory = nav::history,
            openInsights = { openTab(HubTab.INSIGHTS) },
            openCategory = { nav.transactions(categoryId = it) },
            openEntry = { nav.entry(id = it) },
            openBill = { nav.billEdit(it) },
            addBill = { nav.billEdit(0) },
            openAccount = { nav.transactions(accountId = it) },
            accounts = nav::accounts,
            openGoal = { nav.goal(it) },
            addGoal = { nav.goalEdit(0) },
        ),
    )
}

/** Everything Home can do, as plain lambdas, so the screen renders in a test with no graph. */
data class HomeActions(
    val settings: () -> Unit = {},
    val setBudget: () -> Unit = {},
    val openPlan: () -> Unit = {},
    /** Every entry, day by day: Recent's "view all". */
    val openHistory: () -> Unit = {},
    val openInsights: () -> Unit = {},
    val openCategory: (Long) -> Unit = {},
    val openEntry: (Long) -> Unit = {},
    val openBill: (Long) -> Unit = {},
    val addBill: () -> Unit = {},
    val openAccount: (Long) -> Unit = {},
    val accounts: () -> Unit = {},
    val openGoal: (Long) -> Unit = {},
    val addGoal: () -> Unit = {},
)

/**
 * Home: what is left to spend this month in the one lit panel, money in and out beside last month,
 * the week's days against the daily budget, then the envelopes furthest over pace, the goals that
 * need attention, the newest entries by day (History's trim, as Avex's Home keeps its Recent), what
 * is due, and where the money sits. Each section is one panel led by its glyph.
 *
 * The layout follows the width it is given ([homeLayout]): the phone column; from 600dp the same
 * column capped at [READING_WIDTH] and centred, so no reading sits a screen away from its label;
 * from 840dp two panes under one title, the story at the start and the ledger at the end.
 */
@Composable
fun HomeScreen(state: HomeState, actions: HomeActions) {
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val layout = homeLayout(maxWidth)
        // On a phone the add FAB floats over the list's end; wider, it heads the rail, so the list
        // only keeps clear of the system bar.
        val bottom = if (layout == HomeLayout.PHONE) {
            FAB_CLEARANCE
        } else {
            GUTTER + WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
        }
        val lane = when (layout) {
            HomeLayout.PHONE -> Modifier
            HomeLayout.COLUMN -> Modifier.widthIn(max = READING_WIDTH)
            HomeLayout.PANES -> Modifier.widthIn(max = READING_WIDTH * 2)
        }
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = bottom),
            verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            item(key = "head") { HomeHead(state, actions, lane) }
            // Until the first read lands every figure is a placeholder zero: drawing "$0" and "No
            // accounts yet" over a full ledger would say something untrue, so only the frame shows.
            if (state.loaded) {
                if (layout == HomeLayout.PANES) {
                    item(key = "panes") { HomePanes(state, actions, lane) }
                } else {
                    val panel = lane.padding(horizontal = GUTTER)
                    item(key = "left") { LeftToSpendHero(state, actions, panel) }
                    item(key = "inout") { InOutTiles(state, panel) }
                    item(key = "week") { WeekPanel(state, actions, panel) }
                    item(key = "budgets") { BudgetsPanel(state, actions, panel) }
                    item(key = "goals") { GoalsPanel(state, actions, panel) }
                    item(key = "recent") { RecentPanel(state, actions, panel) }
                    item(key = "upcoming") { UpcomingPanel(state, actions, panel) }
                    item(key = "accounts") { AccountsPanel(state, actions, panel) }
                }
            }
        }
    }
}

/** How Home sits in the width it is given. */
internal enum class HomeLayout { PHONE, COLUMN, PANES }

/** The phone column below 600dp, one capped column from 600dp, two panes from 840dp. */
internal fun homeLayout(width: Dp): HomeLayout = when {
    width >= PANES_FROM -> HomeLayout.PANES
    width >= COLUMN_FROM -> HomeLayout.COLUMN
    else -> HomeLayout.PHONE
}

private val COLUMN_FROM = 600.dp
private val PANES_FROM = 840.dp

/** The top bar with its settings capsule, then the month's title and its context line. */
@Composable
private fun HomeHead(state: HomeState, actions: HomeActions, modifier: Modifier = Modifier) {
    Column(modifier) {
        TopBar(onBack = null) { ChromeButton(Icons.Rounded.Settings, "Settings", actions.settings) }
        val context = buildList {
            add(Copy.plural(state.daysLeft, "day") + " left")
            add("resets " + Dates.short(state.period.endExclusive, state.today))
            if (state.sampleLoaded) add("sample data")
        }.joinToString(" · ")
        PageTitle(
            Dates.period(state.period, state.today),
            Modifier.padding(horizontal = GUTTER).padding(top = 4.dp, bottom = 6.dp),
            context = context,
        )
    }
}

/**
 * Two panes on the gutter with the panel gap between: the story in reading order at the start
 * (what is left, money in and out, the week), the ledger at the end (envelopes, recent entries,
 * what is due, the accounts). One title and one scroll serve both.
 */
@Composable
private fun HomePanes(state: HomeState, actions: HomeActions, modifier: Modifier = Modifier) {
    Row(
        modifier.fillMaxWidth().padding(horizontal = GUTTER),
        horizontalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(PANEL_GAP)) {
            LeftToSpendHero(state, actions)
            InOutTiles(state)
            WeekPanel(state, actions)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(PANEL_GAP)) {
            BudgetsPanel(state, actions)
            GoalsPanel(state, actions)
            RecentPanel(state, actions)
            UpcomingPanel(state, actions)
            AccountsPanel(state, actions)
        }
    }
}

/**
 * What is left of the month's budget, drawn: the screen's one serif hero under the accent's
 * light, the budget meter with today's pace tick, and chips for the readings that say what to do.
 * Without a budget it measures against income, honestly labelled, and offers the budget.
 */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun LeftToSpendHero(state: HomeState, actions: HomeActions, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val r = state.reading
    val over = r?.status == PaceStatus.OVER_BUDGET
    val tint = if (over) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary
    HeroPanel(modifier) {
        // The kit's header: the pace reading drops under the label before the label would wrap.
        PanelHeader(
            if (over) "Over budget" else if (r != null) "Left to spend" else "Left from income",
            meta = if (r != null && !over) Copy.paceLine(r, money) else null,
            tint = tint,
        )
        Spacer(Modifier.height(14.dp))
        if (r != null) {
            Text(
                money.formatWhole(if (over) -r.remaining else r.remaining),
                style = MaterialTheme.typography.displayLarge,
                color = MaterialTheme.colorScheme.onBackground,
            )
            Text(
                (if (over) "over your " else "left of your ") + money.formatWhole(r.budget) + " budget · " +
                    money.formatWhole(r.spent) + " spent",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(16.dp))
            PaceMeter(
                r.spentFraction,
                r.paceFraction,
                "Spent ${money.formatWhole(r.spent)} of ${money.formatWhole(r.budget)}. " +
                    "An even pace would be ${money.formatWhole(r.expected)} by today.",
                height = 14.dp,
            )
            Spacer(Modifier.height(16.dp))
            // The days left are in the context line under the title; the allowance says them in
            // the same breath ("$49 a day for 28 days") instead of repeating them as a chip.
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (!over) StatChip(Icons.Rounded.Speed, Copy.marginLine(r, money))
                StatChip(Icons.AutoMirrored.Rounded.TrendingUp, "Pace " + money.formatWhole(r.expected))
            }
        } else {
            val fraction = if (state.income > 0) state.spent.toFloat() / state.income else 0f
            Text(
                money.formatWhole(state.income - state.spent),
                style = MaterialTheme.typography.displayLarge,
                color = MaterialTheme.colorScheme.onBackground,
            )
            Text(
                money.formatWhole(state.spent) + " spent of " + money.formatWhole(state.income) + " in",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(16.dp))
            PaceMeter(
                fraction,
                null,
                if (state.income > 0) "Spent ${money.formatWhole(state.spent)} of ${money.formatWhole(state.income)} income"
                else "No budget and no income this month",
                height = 14.dp,
            )
            Spacer(Modifier.height(10.dp))
            AsideAction("A monthly budget adds the pace tick", "set budget", actions.setBudget)
        }
    }
}

/**
 * A zero state's quiet line with the act that fills it at the end. Where the two cannot share a
 * line without breaking a word of the line (200% font), the act drops under it.
 */
@Composable
private fun AsideAction(text: String, action: String, onAction: () -> Unit) {
    EndsRow(
        start = { Aside(text) },
        end = { TextAction(action, onAction, color = MaterialTheme.colorScheme.primary) },
        modifier = Modifier.fillMaxWidth(),
    )
}

/** Money in and out, each beside where last month stood by the same day. */
@Composable
private fun InOutTiles(state: HomeState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    fun versus(now: Long, then: Long): String = when {
        then <= 0L -> "Nothing by this day last month"
        now == then -> "Level with last month"
        now > then -> money.formatWhole(now - then) + " more than last month"
        else -> money.formatWhole(then - now) + " less than last month"
    }
    Row(modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(FIGURE_GAP)) {
        StatTile(
            "In",
            money.formatWhole(state.income),
            Modifier.weight(1f).fillMaxHeight(),
            detail = versus(state.income, state.lastPeriodIncomeSameDay),
        )
        StatTile(
            "Out",
            money.formatWhole(state.spent),
            Modifier.weight(1f).fillMaxHeight(),
            detail = versus(state.spent, state.lastPeriodSameDay),
        )
    }
}

@Composable
private fun WeekPanel(state: HomeState, actions: HomeActions, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val labels = List(7) { Dates.weekdayInitial(state.weekStart.plusDays(it.toLong()).dayOfWeek) }
    val before = state.weekSpentBeforePeriod
    val desc = weekDescription(state.weekStart, state.week, state.todayInWeek, before, state.period.start) { money.formatWhole(it) }
    // Early in a month the week opens in the last one. What those days spent is named, so the
    // week's figure and the month's Out add up on screen.
    val daily = state.dailyBudget
    val line = buildList {
        add(money.formatWhole(state.weekSpent) + " spent")
        if (before > 0L) add(money.formatWhole(before) + " before " + Dates.short(state.period.start, state.today))
        if (daily != null) add(money.formatWhole(daily) + " a day budgeted")
    }.joinToString(" · ")
    Panel(modifier) {
        PanelHeader(
            "This week",
            action = "insights",
            onAction = actions.openInsights,
        )
        Spacer(Modifier.height(6.dp))
        Text(line, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(14.dp))
        DayBars(
            state.week,
            labels,
            state.todayInWeek,
            desc,
            allowance = state.dailyBudget,
            beforeCount = state.weekDaysBeforePeriod,
        )
    }
}

@Composable
private fun BudgetsPanel(state: HomeState, actions: HomeActions, modifier: Modifier = Modifier) {
    Panel(modifier) {
        PanelHeader(
            "Budgets",
            action = if (state.envelopes.isEmpty()) null else "all",
            onAction = actions.openPlan,
        )
        if (state.envelopes.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            AsideAction("No category budgets yet", "add one", actions.openPlan)
        } else {
            state.envelopes.take(4).forEach { e -> EnvelopeRow(e) { actions.openCategory(e.categoryId) } }
        }
    }
}

@Composable
private fun EnvelopeRow(e: Envelope, onClick: () -> Unit) {
    val money = LocalMoney.current
    val r = e.reading
    val reading = Copy.ofBudget(r.spent, r.budget, money)
    Column(
        Modifier
            .fillMaxWidth()
            .bounceClick(label = "${e.name} entries", focusOutset = ROW_FOCUS_OUTSET, onClick = onClick)
            .heightIn(min = 56.dp)
            .padding(top = 12.dp, bottom = 4.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            CategoryBadge(e.icon, e.color, size = 36.dp)
            EndsRow(
                start = {
                    Column {
                        Text(e.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                        Text(Copy.paceLine(r, money), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                },
                end = { Text(reading, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground) },
                modifier = Modifier.weight(1f),
            )
        }
        Spacer(Modifier.height(10.dp))
        PaceMeter(r.spentFraction, r.paceFraction, "${e.name}: $reading. ${Copy.paceLine(r, money)}", height = 6.dp)
    }
}

/**
 * The goals that need attention, Avex's Home section in Tally's panel: behind first, reached last,
 * three at most, each opening its goal.
 */
@Composable
private fun GoalsPanel(state: HomeState, actions: HomeActions, modifier: Modifier = Modifier) {
    Panel(modifier) {
        PanelHeader(
            "Goals",
            action = if (state.goals.isEmpty()) null else "all",
            onAction = actions.openPlan,
        )
        if (state.goals.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            AsideAction("A goal: a pot, an amount by a date, or a share of every month invested", "add one", actions.addGoal)
        } else {
            state.goals.forEach { g -> GoalRow(g, state.today) { actions.openGoal(g.id) } }
        }
    }
}

/**
 * The newest entries, grouped under their day as History groups them, with "view all" opening
 * History itself. The day says when once, so the rows carry only where.
 */
@Composable
private fun RecentPanel(state: HomeState, actions: HomeActions, modifier: Modifier = Modifier) {
    Panel(modifier) {
        PanelHeader(
            "Recent",
            action = if (state.recent.isEmpty()) null else "view all",
            onAction = actions.openHistory,
        )
        if (state.recent.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            Aside("Your first entry lands here. Tap + to log one")
        } else {
            state.recent.groupBy { it.date }.forEach { (date, rows) ->
                Text(
                    Dates.day(date, state.today).uppercase(),
                    style = MaterialTheme.typography.labelMedium,
                    color = if (date == state.today) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 12.dp).semantics { heading() },
                )
                rows.forEach { row -> LedgerRow(row, meta = "", onClick = { actions.openEntry(row.id) }) }
            }
        }
    }
}

@Composable
private fun UpcomingPanel(state: HomeState, actions: HomeActions, modifier: Modifier = Modifier) {
    Panel(modifier) {
        PanelHeader("Upcoming", meta = if (state.upcoming.isEmpty()) null else "NEXT 14 DAYS")
        if (state.upcoming.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            AsideAction("Nothing due in the next two weeks", "add a bill", actions.addBill)
        } else {
            Spacer(Modifier.height(4.dp))
            state.upcoming.forEach { b -> BillRow(b) { actions.openBill(b.id) } }
        }
    }
}

@Composable
private fun BillRow(b: UpcomingBill, onClick: () -> Unit) {
    val money = LocalMoney.current
    val amount = if (b.type == TxType.INCOME) money.formatSigned(b.amount) else money.format(b.amount)
    val meta = listOf(Copy.dueLine(b.daysUntil), b.accountName).joinToString(" · ")
    Row(
        Modifier
            .fillMaxWidth()
            .bounceClick(label = "Edit ${b.name}", focusOutset = ROW_FOCUS_OUTSET, onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = "${b.name}, $amount, $meta" }
            .heightIn(min = 64.dp)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        if (b.type == TxType.TRANSFER) TransferBadge() else CategoryBadge(b.icon, b.color)
        EndsRow(
            start = {
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(b.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                    Text(meta, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            },
            end = { Text(amount, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onBackground) },
            modifier = Modifier.weight(1f).clearAndSetSemantics { },
            gap = 14.dp,
        )
    }
}

@Composable
private fun AccountsPanel(state: HomeState, actions: HomeActions, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val active = state.accounts.filter { !it.archived }
    Panel(modifier) {
        PanelHeader("Accounts", action = "manage", onAction = actions.accounts)
        if (active.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            Aside("No accounts yet")
        } else {
            Spacer(Modifier.height(10.dp))
            Text(money.format(state.net), style = MaterialTheme.typography.headlineMedium, color = MaterialTheme.colorScheme.onBackground)
            Text(netAcrossLabel(active.size), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(6.dp))
            active.take(5).forEach { a ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .bounceClick(label = "${a.name} entries", focusOutset = ROW_FOCUS_OUTSET) { actions.openAccount(a.id) }
                        .heightIn(min = 60.dp)
                        .padding(vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    AccountBadge(a.type)
                    EndsRow(
                        start = {
                            Column {
                                Text(a.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                                Text(
                                    Copy.plural(a.entryCount, "entry", "entries"),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        },
                        end = {
                            Text(money.format(a.balance), style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onBackground)
                        },
                        modifier = Modifier.weight(1f),
                        gap = 14.dp,
                    )
                }
            }
        }
    }
}

/**
 * The week chart's spoken reading, each day so far by its full name: initials would read
 * Tuesday and Thursday, Saturday and Sunday, as the same letter. When the week opens before the
 * period began, it ends with what those days spent ("$197 of it before 1 October").
 */
internal fun weekDescription(
    weekStart: LocalDate,
    week: List<Long>,
    todayInWeek: Int,
    spentBeforePeriod: Long = 0L,
    periodStart: LocalDate = weekStart,
    format: (Long) -> String,
): String {
    val days = "This week: " + week.take(todayInWeek + 1).mapIndexed { i, v ->
        Dates.weekday(weekStart.plusDays(i.toLong()).dayOfWeek) + " " + format(v)
    }.joinToString(", ")
    if (spentBeforePeriod <= 0L) return days
    return days + ", " + format(spentBeforePeriod) + " of it before " + periodStart.dayOfMonth + " " + Dates.month(periodStart)
}

/** "NET ACROSS 1 ACCOUNT", "NET ACROSS 4 ACCOUNTS": onboarding makes exactly one. */
internal fun netAcrossLabel(accounts: Int): String = "NET ACROSS " + Copy.plural(accounts, "account").uppercase()
