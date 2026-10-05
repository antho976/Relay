package com.tally.app.ui.activity

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ReceiptLong
import androidx.compose.material.icons.automirrored.rounded.TrendingUp
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.CalendarToday
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.ui.common.Aside
import com.tally.app.ui.common.Caption
import com.tally.app.ui.common.CategoryIcons
import com.tally.app.ui.common.ChromeButton
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.EndsRow
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PaceMeter
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.StatTile
import com.tally.app.ui.common.TextAction
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.categoryColor
import com.tally.core.CategoryKind
import com.tally.core.Copy
import com.tally.core.MoneyFormatter
import com.tally.core.PaceReading
import com.tally.core.PaceStatus
import com.tally.core.TxType

/** One category's or one account's entries, all time (route args category / account, 0 = unset). */
@Composable
fun TransactionsRoute(nav: AppNav) {
    val viewModel: TransactionsViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    var text by rememberSaveable { mutableStateOf("") }
    LaunchedEffect(text) { viewModel.setQuery(text) }
    LifecycleResumeEffect(viewModel) {
        viewModel.onResume()
        onPauseOrDispose { }
    }
    val s = state
    TransactionsScreen(
        state = s,
        query = text,
        actions = TransactionsActions(
            back = nav::back,
            onQuery = { text = it },
            edit = {
                if (s.isAccount) nav.accountEdit(s.accountId) else nav.categoryEdit(s.categoryId, s.categoryKind)
            },
            add = {
                if (s.isAccount) {
                    nav.entry(accountId = s.accountId)
                } else {
                    val type = if (s.categoryKind == CategoryKind.INCOME) TxType.INCOME else TxType.EXPENSE
                    nav.entry(type = type, categoryId = s.categoryId)
                }
            },
            editBudget = { nav.budgetEdit(s.categoryId) },
            openEntry = { nav.entry(id = it) },
        ),
    )
}

/** Everything the screen can do, as plain lambdas, so it renders in a test with no graph. */
data class TransactionsActions(
    val back: () -> Unit = {},
    val onQuery: (String) -> Unit = {},
    val edit: () -> Unit = {},
    val add: () -> Unit = {},
    val editBudget: () -> Unit = {},
    val openEntry: (Long) -> Unit = {},
)

/**
 * One envelope or one account as a ledger: this period's reading in the lit panel (the budget
 * and its pace for a category, the money through it for an account), two readings as tiles, then
 * every entry day by day with the search over them.
 */
@Composable
fun TransactionsScreen(state: TransactionsState, query: String, actions: TransactionsActions) {
    val money = LocalMoney.current
    val periodName = remember(state.period, state.today) { Dates.period(state.period, state.today) }
    val bottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    val noun = if (state.isAccount) "account" else "category"
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = bottom + 32.dp),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        item(key = "head") {
            Column {
                TopBar(onBack = actions.back) {
                    if (state.loaded && !state.missing) {
                        ChromeButton(Icons.Rounded.Add, "Add entry", actions.add)
                        ChromeButton(Icons.Rounded.Edit, "Edit $noun", actions.edit)
                    }
                }
                PageTitle(
                    if (state.missing) "Not found" else state.name.orEmpty(),
                    Modifier.padding(horizontal = GUTTER).padding(top = 4.dp, bottom = 6.dp),
                    context = headContext(state, money),
                )
            }
        }
        if (state.missing) {
            item(key = "missing") {
                EmptyPanel(
                    if (state.isAccount) "Its entries were removed with it" else "Its entries are still in Activity",
                    Modifier.padding(horizontal = GUTTER),
                )
            }
        } else if (state.loaded) {
            item(key = "hero") {
                val m = Modifier.padding(horizontal = GUTTER)
                val reading = state.reading
                when {
                    state.isAccount -> AccountHero(state, periodName, m)
                    state.categoryKind == CategoryKind.INCOME -> IncomeHero(state, periodName, m)
                    reading != null -> BudgetHero(state, reading, periodName, actions.editBudget, m)
                    else -> NoBudgetHero(state, periodName, actions.editBudget, m)
                }
            }
            item(key = "tiles") {
                if (state.isAccount) AccountTiles(state, Modifier.padding(horizontal = GUTTER))
                else CategoryTiles(state, Modifier.padding(horizontal = GUTTER))
            }
            item(key = "controls") {
                Column(Modifier.padding(horizontal = GUTTER), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    SearchField(
                        query,
                        actions.onQuery,
                        placeholder = if (state.isAccount) "Search notes and categories" else "Search notes and accounts",
                    )
                    if (state.truncated) {
                        Caption("Showing the newest ${LedgerRepository.SEARCH_LIMIT}", Modifier.padding(horizontal = 4.dp))
                    }
                }
            }
            if (state.days.isEmpty()) {
                item(key = "empty") {
                    if (state.searching) {
                        EmptyPanel(
                            "No entries match \"${state.query}\"",
                            Modifier.padding(horizontal = GUTTER),
                            action = "clear search",
                            onAction = { actions.onQuery("") },
                        )
                    } else {
                        EmptyPanel(
                            "Nothing logged in ${state.name.orEmpty()} yet",
                            Modifier.padding(horizontal = GUTTER),
                            action = "log one",
                            onAction = actions.add,
                        )
                    }
                }
            } else {
                items(state.days, key = { "day-" + it.date.toEpochDay() }) { group ->
                    DayPanel(group, state.today, actions.openEntry, Modifier.padding(horizontal = GUTTER))
                }
            }
        }
    }
}

/** The line under the title: how many entries, and the balance or the standing budget. */
private fun headContext(state: TransactionsState, money: MoneyFormatter): String? {
    if (!state.loaded) return null
    if (state.missing) return if (state.isAccount) "This account was deleted" else "This category was deleted"
    val entries = Copy.plural(state.entryCount, "entry", "entries")
    return when {
        state.isAccount -> "$entries · balance ${money.format(state.balance)}"
        state.categoryKind == CategoryKind.INCOME -> "$entries · income"
        state.budget != null -> "$entries · ${money.formatWhole(state.budget)} a month budget"
        else -> entries
    }
}

private fun dayOf(state: TransactionsState): String =
    "DAY ${state.period.elapsedDays(state.today)} OF ${state.period.days}"

/** "$12 more than last month by this day", for either direction of money. */
private fun versusLine(now: Long, then: Long, money: MoneyFormatter): String = when {
    then <= 0L -> "Nothing by this day last month"
    now == then -> "Level with last month by this day"
    now > then -> money.formatWhole(now - then) + " more than last month by this day"
    else -> money.formatWhole(then - now) + " less than last month by this day"
}

/** A budgeted envelope: what is left in the display serif, the meter with today's pace tick. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun BudgetHero(
    state: TransactionsState,
    r: PaceReading,
    periodName: String,
    onEditBudget: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val money = LocalMoney.current
    val over = r.status == PaceStatus.OVER_BUDGET
    val tint = if (over) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary
    val reading = Copy.ofBudget(r.spent, r.budget, money)
    HeroPanel(modifier) {
        HeroLabel(
            periodName,
            meta = if (over) "OVER BUDGET" else Copy.paceLine(r, money).uppercase(),
            iconTint = categoryColor(state.color),
        )
        Spacer(Modifier.height(14.dp))
        Text(
            money.formatWhole(if (over) -r.remaining else r.remaining),
            style = MaterialTheme.typography.displayLarge,
            color = MaterialTheme.colorScheme.onBackground,
        )
        Text(
            (if (over) "over · " else "left · ") + reading + " spent",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        PaceMeter(
            r.spentFraction,
            r.paceFraction,
            "${state.name.orEmpty()}: $reading. An even pace would be ${money.formatWhole(r.expected)} by today.",
            height = 14.dp,
        )
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.Rounded.CalendarToday, Copy.plural(r.daysLeft, "day") + " left")
            if (!over) StatChip(Icons.Rounded.Speed, money.formatWhole(r.dailyAllowance) + " a day")
            StatChip(Icons.AutoMirrored.Rounded.TrendingUp, "Pace " + money.formatWhole(r.expected))
            StatChip(Icons.AutoMirrored.Rounded.ReceiptLong, Copy.plural(state.periodCount, "entry", "entries"))
        }
        Spacer(Modifier.height(6.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            TextAction("edit budget", onEditBudget, color = MaterialTheme.colorScheme.primary)
        }
    }
}

/** An envelope with no budget: the period's spend, honestly unmeasured, and the way to measure it. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun NoBudgetHero(state: TransactionsState, periodName: String, onSetBudget: () -> Unit, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val elapsed = state.period.elapsedDays(state.today).coerceAtLeast(1)
    HeroPanel(modifier) {
        HeroLabel(periodName, meta = "NO BUDGET", iconTint = categoryColor(state.color))
        Spacer(Modifier.height(14.dp))
        Text(
            money.formatWhole(state.periodTotal),
            style = MaterialTheme.typography.displayLarge,
            color = MaterialTheme.colorScheme.onBackground,
        )
        Text(
            "spent across " + Copy.plural(state.periodCount, "entry", "entries"),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.Rounded.Speed, money.formatWhole(state.periodTotal / elapsed) + " a day")
            StatChip(Icons.Rounded.CalendarToday, Copy.plural(state.period.daysLeft(state.today), "day") + " left")
        }
        Spacer(Modifier.height(10.dp))
        // The act drops under the line before a word of the line would break (200% font).
        EndsRow(
            start = { Aside("A budget adds a meter and a pace tick") },
            end = { TextAction("set budget", onSetBudget, color = MaterialTheme.colorScheme.primary) },
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/** An income category: what came in this period, in the colour of money in. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun IncomeHero(state: TransactionsState, periodName: String, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    HeroPanel(modifier) {
        HeroLabel(periodName, meta = "CAME IN", iconTint = categoryColor(state.color))
        Spacer(Modifier.height(14.dp))
        Text(
            money.formatWhole(state.periodTotal),
            style = MaterialTheme.typography.displayLarge,
            color = MaterialTheme.colorScheme.tertiary,
        )
        Text(
            "across " + Copy.plural(state.periodCount, "entry", "entries"),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.Rounded.History, "Last month " + money.formatWhole(state.lastPeriodTotal))
            StatChip(Icons.Rounded.CalendarToday, Copy.plural(state.period.daysLeft(state.today), "day") + " left")
        }
    }
}

/** An account: this period's money out of it and into it. */
@Composable
private fun AccountHero(state: TransactionsState, periodName: String, modifier: Modifier = Modifier) {
    val f = state.flows
    FlowHero(
        label = periodName,
        meta = dayOf(state),
        spent = f.moneyOut,
        income = f.moneyIn,
        chips = listOf(
            HeroChip(
                Icons.AutoMirrored.Rounded.ReceiptLong,
                Copy.plural(f.inCount + f.outCount, "entry", "entries") + " this month",
            ),
        ),
        modifier = modifier,
    )
}

@Composable
private fun CategoryTiles(state: TransactionsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val isIn = state.categoryKind == CategoryKind.INCOME
    val tint = if (isIn) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.primary
    val s = state.summary
    TilePair(
        modifier,
        first = { m ->
            StatTile(
                "Last month",
                money.formatWhole(state.lastPeriodTotal),
                m,
                detail = versusLine(state.periodTotal, state.lastPeriodSameDay, money),
            )
        },
        second = { m ->
            StatTile(
                "Per entry",
                money.format(s.average),
                m,
                detail = if (s.basisCount > 0) "Across " + Copy.plural(s.basisCount, "entry", "entries") else "Nothing logged yet",
            )
        },
    )
}

@Composable
private fun AccountTiles(state: TransactionsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val change = state.balance - state.openingBalance
    val up = change > 0
    TilePair(
        modifier,
        first = { m ->
            StatTile(
                "Opened with",
                money.format(state.openingBalance),
                m,
                detail = accountTypeLabel(state.accountType),
            )
        },
        second = { m ->
            StatTile(
                "Since opening",
                money.formatSigned(change),
                m,
                detail = "Across " + Copy.plural(state.entryCount, "entry", "entries"),
                valueColor = if (up) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.onBackground,
            )
        },
    )
}
