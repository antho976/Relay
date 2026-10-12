package com.tally.app.ui.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
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
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.kit.AccountBadge
import com.tally.app.ui.kit.Balance
import com.tally.app.ui.kit.Empty
import com.tally.app.ui.kit.Eyebrow
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.PaceMeter
import com.tally.app.ui.kit.Panel
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.SpacePage
import com.tally.app.ui.kit.Stat
import com.tally.app.ui.kit.StatGrid
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.WeekBars
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.plan.BillRow
import com.tally.app.ui.plan.BudgetRow
import com.tally.app.ui.plan.GoalRow
import com.tally.app.ui.plan.paceTone
import com.tally.app.ui.theme.Tally
import com.tally.core.Copy
import com.tally.core.PaceStatus
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
        ),
    )
}

/**
 * The month at a glance, as Relay draws a source: what is left with its pace in words and on the
 * meter, what each remaining day can take, the week, the budgets furthest over pace, the bills
 * coming up, goals, recent entries and the accounts.
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
            Slab {
                Empty(
                    "wallet",
                    "Nothing logged yet",
                    "Add an account and log what you spend; Tally keeps the month's pace from there. Or import a bank statement.",
                ) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Key("Add an account", a.addAccount, glyph = "bank")
                        Key("Import", a.import, glyph = "download", kind = KeyKind.Quiet)
                    }
                }
            }
            return@SpacePage
        }

        // The reading: what is left, its pace, and what each day can take.
        Slab(Modifier.fillMaxWidth(), padding = PaddingValues(16.dp), spacing = 10.dp) {
            if (r != null) {
                Eyebrow(if (r.status == PaceStatus.OVER_BUDGET) "Over budget" else "Left this month")
                T(
                    fmt.formatWhole(if (r.remaining < 0) -r.remaining else r.remaining),
                    style = Tally.type.hero,
                    color = if (r.remaining < 0) c.heldText else c.ink,
                    maxLines = 1,
                )
                T(Copy.daysLine(r), style = Tally.type.uiMedium, color = paceTone(r).takeIf { r.status != PaceStatus.ON_PACE && r.status != PaceStatus.UNDER_PACE } ?: c.liveText)
                PaceMeter(r.spentFraction, r.paceFraction, height = 8.dp)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    T(Copy.ofBudget(r.spent, r.budget, fmt) + " spent", Modifier.weight(1f), Tally.type.caption, c.ink3, maxLines = 1)
                    T(Copy.marginLine(r, fmt), style = Tally.type.caption, color = c.ink2, maxLines = 1)
                }
            } else {
                Eyebrow("Spent this month")
                T(fmt.formatWhole(s.spent), style = Tally.type.hero, color = c.ink, maxLines = 1)
                T("Set a monthly budget and Tally reads the month as a pace: how much is left, and what each day can take.", style = Tally.type.caption, color = c.ink3)
                Key("Set a monthly budget", a.setBudget, glyph = "pie", compact = true)
            }
        }

        StatGrid(
            listOf(
                { m -> Stat("Money in", fmt.formatWhole(s.income), m, glyph = "money-in", sub = versus(s.income, s.lastPeriodIncomeSameDay, fmt, "in")) },
                { m -> Stat("Money out", fmt.formatWhole(s.spent), m, glyph = "money-out", sub = Copy.versusLastLine(s.spent, s.lastPeriodSameDay, fmt)) },
                { m -> Stat("Net worth", fmt.formatWhole(s.net), m, glyph = "bank", figureColor = if (s.net < 0) c.heldText else c.ink, sub = Copy.plural(s.accounts.count { !it.archived }, "account"), onClick = a.accounts) },
                { m -> Stat("Each day can take", if (r != null) fmt.formatWhole(r.dailyAllowance) else "–", m, glyph = "calendar", sub = if (r != null) "for " + Copy.plural(r.daysLeft, "day") else "with a monthly budget") },
            ),
        )

        Panel("This week", glyph = "calendar", subtitle = fmt.formatWhole(s.weekSpent) + " spent") {
            val labels = List(7) { Dates.weekdayInitial(s.weekStart.plusDays(it.toLong()).dayOfWeek) }
            val description = weekDescription(s.weekStart, s.week, s.todayInWeek, s.weekSpentBeforePeriod, s.period.start) { fmt.formatWhole(it) }
            WeekBars(s.week, s.todayInWeek, labels, description, Modifier.padding4(), daily = s.dailyBudget)
        }

        if (s.envelopes.isNotEmpty()) {
            Panel("Budgets", glyph = "pie", action = "All", onAction = a.budgets) {
                s.envelopes.take(4).forEach { e -> BudgetRow(e.name, e.icon, e.color, e.reading, onClick = { a.budget(e.categoryId) }) }
            }
        }

        if (s.upcoming.isNotEmpty()) {
            Panel("Coming up", glyph = "repeat", action = "Bills", onAction = a.bills) {
                s.upcoming.forEach { b ->
                    BillRow(
                        b.name, b.type, b.amount, b.icon, b.color,
                        line = Copy.dueLine(b.daysUntil) + " · " + b.accountName + if (b.autoPost) "" else " · reminder",
                        onClick = { a.bill(b.id) },
                        due = b.daysUntil,
                        autoPost = b.autoPost,
                    )
                }
            }
        }

        if (s.goals.isNotEmpty()) {
            Panel("Goals", glyph = "target", action = "All", onAction = a.goals) {
                s.goals.forEach { g -> GoalRow(g, s.today, onClick = { a.goal(g.id) }) }
            }
        }

        Panel("Recent", glyph = "list", action = "All", onAction = a.entries) {
            if (s.recent.isEmpty()) {
                T("Nothing logged this month yet.", Modifier.padding4(), Tally.type.caption, c.ink3)
            }
            s.recent.forEach { row -> EntryRow(row, onClick = { a.entry(row.id) }, onLongClick = { a.deleteEntry(row.id) }, today = s.today) }
        }

        Panel("Accounts", glyph = "bank", action = "All", onAction = a.accounts) {
            s.accounts.filter { !it.archived }.forEach { acc ->
                ListRow(onClick = { a.account(acc.id) }, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
                    AccountBadge(acc.type)
                    Column(Modifier.weight(1f)) {
                        T(acc.name, style = Tally.type.ui, color = c.ink, maxLines = 1)
                        T(Copy.plural(acc.entryCount, "entry", "entries"), style = Tally.type.caption, color = c.ink3, maxLines = 1)
                    }
                    Balance(acc.balance)
                }
            }
        }
    }
}

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
