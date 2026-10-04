package com.margin.app.ui.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.margin.app.data.db.TransactionRow
import com.margin.app.ui.common.AccountBadge
import com.margin.app.ui.common.Aside
import com.margin.app.ui.common.CategoryBadge
import com.margin.app.ui.common.ChromeButton
import com.margin.app.ui.common.Dates
import com.margin.app.ui.common.GUTTER
import com.margin.app.ui.common.LedgerRow
import com.margin.app.ui.common.LocalMoney
import com.margin.app.ui.common.PaceMeter
import com.margin.app.ui.common.PageTitle
import com.margin.app.ui.common.SectionHeader
import com.margin.app.ui.common.TextAction
import com.margin.app.ui.common.TopBar
import com.margin.app.ui.common.TransferBadge
import com.margin.app.ui.common.bounceClick
import com.margin.app.ui.nav.AppNav
import com.margin.app.ui.nav.FAB_CLEARANCE
import com.margin.app.ui.nav.HubTab
import com.margin.core.Copy
import com.margin.core.PaceStatus
import com.margin.core.TxType

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
            openActivity = { openTab(HubTab.ACTIVITY) },
            openCategory = { nav.transactions(categoryId = it) },
            openEntry = { nav.entry(id = it) },
            openBill = { nav.billEdit(it) },
            addBill = { nav.billEdit(0) },
            openAccount = { nav.transactions(accountId = it) },
            accounts = nav::accounts,
        ),
    )
}

/** Everything Home can do, as plain lambdas, so the screen renders in a test with no graph. */
data class HomeActions(
    val settings: () -> Unit = {},
    val setBudget: () -> Unit = {},
    val openPlan: () -> Unit = {},
    val openActivity: () -> Unit = {},
    val openCategory: (Long) -> Unit = {},
    val openEntry: (Long) -> Unit = {},
    val openBill: (Long) -> Unit = {},
    val addBill: () -> Unit = {},
    val openAccount: (Long) -> Unit = {},
    val accounts: () -> Unit = {},
)

/**
 * Home, the in / out ledger (surface brief, direction contract): the month's two sides, the
 * margin between them drawn as the budget meter with its pace tick, then the envelopes furthest
 * ahead of pace, the newest entries, what is due, and where the money sits.
 */
@Composable
fun HomeScreen(state: HomeState, actions: HomeActions) {
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = FAB_CLEARANCE),
    ) {
        item(key = "bar") {
            TopBar(onBack = null) { ChromeButton(Icons.Rounded.Settings, "Settings", actions.settings) }
        }
        item(key = "title") {
            val context = buildList {
                add(Copy.plural(state.daysLeft, "day") + " left")
                add("resets " + Dates.short(state.period.endExclusive, state.today))
                if (state.sampleLoaded) add("sample data")
            }.joinToString(" · ")
            PageTitle(
                Dates.period(state.period, state.today),
                Modifier.padding(horizontal = GUTTER).padding(top = 8.dp),
                context = context,
            )
        }
        item(key = "ledger") { LedgerPair(state, Modifier.padding(horizontal = GUTTER).padding(top = 28.dp)) }
        item(key = "margin") { MarginBlock(state, actions, Modifier.padding(horizontal = GUTTER).padding(top = 20.dp)) }

        envelopes(state, actions)
        recent(state, actions)
        upcoming(state, actions)
        accounts(state, actions)
    }
}

/** IN at the start, OUT at the end: the two sides of the month. */
@Composable
private fun LedgerPair(state: HomeState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(money.formatWhole(state.income), style = MaterialTheme.typography.headlineMedium, color = MaterialTheme.colorScheme.onBackground)
            Text("IN", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Column(Modifier.weight(1f), horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                money.formatWhole(state.spent),
                style = MaterialTheme.typography.headlineMedium,
                color = MaterialTheme.colorScheme.onBackground,
                textAlign = TextAlign.End,
            )
            Text("OUT", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/**
 * The margin, drawn: the meter on the budget's scale with today's pace tick, the margin itself as
 * the screen's one serif hero, and the reading that says what each day can take. Without a
 * budget it falls back to income, honestly labelled, and offers the budget.
 */
@Composable
private fun MarginBlock(state: HomeState, actions: HomeActions, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val r = state.reading
    Column(modifier.fillMaxWidth()) {
        if (r != null) {
            val desc = "Spent ${money.formatWhole(r.spent)} of ${money.formatWhole(r.budget)}. " +
                "An even pace would be ${money.formatWhole(r.expected)} by today."
            PaceMeter(r.spentFraction, r.paceFraction, desc, height = 14.dp)
            Spacer(Modifier.height(18.dp))
            val over = r.status == PaceStatus.OVER_BUDGET
            Text(
                money.formatWhole(if (over) -r.remaining else r.remaining),
                style = MaterialTheme.typography.displayMedium,
                color = if (over) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onBackground,
            )
            Text(
                (if (over) "OVER BUDGET" else "MARGIN") + " · " + money.formatWhole(r.budget).uppercase() + " BUDGET",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(12.dp))
            Text(Copy.marginLine(r, money), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
            if (r.status != PaceStatus.OVER_BUDGET) {
                Text(Copy.paceLine(r, money), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        } else {
            // No budget: the bar measures spending against income, with no pace to claim.
            val fraction = if (state.income > 0) state.spent.toFloat() / state.income else 0f
            val desc = if (state.income > 0) "Spent ${money.formatWhole(state.spent)} of ${money.formatWhole(state.income)} income"
            else "No budget and no income this month"
            PaceMeter(fraction, null, desc, height = 14.dp)
            Spacer(Modifier.height(18.dp))
            Text(
                money.formatWhole(state.income - state.spent),
                style = MaterialTheme.typography.displayMedium,
                color = MaterialTheme.colorScheme.onBackground,
            )
            Text("LEFT FROM INCOME", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(4.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Aside("A monthly budget adds the pace tick", Modifier.weight(1f, fill = false))
                TextAction("set budget", actions.setBudget, color = MaterialTheme.colorScheme.primary)
            }
        }
    }
}

private fun LazyListScope.section(key: String, content: @Composable () -> Unit) {
    item(key = key) {
        Column(Modifier.padding(horizontal = GUTTER).padding(top = 32.dp)) { content() }
    }
}

private fun LazyListScope.envelopes(state: HomeState, actions: HomeActions) {
    section("budgets_header") {
        SectionHeader(
            "Budgets",
            action = if (state.envelopes.isEmpty()) null else "all",
            onAction = actions.openPlan,
        )
        if (state.envelopes.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Aside("No category budgets yet", Modifier.weight(1f, fill = false))
                TextAction("add one", actions.openPlan, color = MaterialTheme.colorScheme.primary)
            }
        }
    }
    items(state.envelopes.take(4), key = { "env_${it.categoryId}" }) { e ->
        EnvelopeRow(e, Modifier.padding(horizontal = GUTTER)) { actions.openCategory(e.categoryId) }
    }
}

@Composable
private fun EnvelopeRow(e: Envelope, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val money = LocalMoney.current
    val r = e.reading
    val reading = Copy.ofBudget(r.spent, r.budget, money)
    Column(
        modifier
            .fillMaxWidth()
            .bounceClick(label = "${e.name} entries", onClick = onClick)
            .heightIn(min = 56.dp)
            .padding(top = 12.dp, bottom = 6.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            CategoryBadge(e.icon, e.color, size = 32.dp)
            Text(e.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.weight(1f))
            Text(reading, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground)
        }
        Spacer(Modifier.height(8.dp))
        PaceMeter(
            r.spentFraction,
            r.paceFraction,
            "${e.name}: $reading. ${Copy.paceLine(r, money)}",
            height = 6.dp,
        )
    }
}

private fun LazyListScope.recent(state: HomeState, actions: HomeActions) {
    section("recent_header") {
        SectionHeader("Recent", action = if (state.recent.isEmpty()) null else "view all", onAction = actions.openActivity)
        if (state.recent.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            Aside("Your first entry lands here")
        } else {
            Spacer(Modifier.height(4.dp))
        }
    }
    items(state.recent, key = { "tx_${it.id}" }) { row: TransactionRow ->
        LedgerRow(row, Dates.day(row.date, state.today), onClick = { actions.openEntry(row.id) }, modifier = Modifier.padding(horizontal = GUTTER))
    }
}

private fun LazyListScope.upcoming(state: HomeState, actions: HomeActions) {
    section("upcoming_header") {
        SectionHeader("Upcoming", meta = if (state.upcoming.isEmpty()) null else "NEXT 14 DAYS")
        if (state.upcoming.isEmpty()) {
            Spacer(Modifier.height(8.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Aside("Nothing due in the next two weeks", Modifier.weight(1f, fill = false))
                TextAction("add a bill", actions.addBill, color = MaterialTheme.colorScheme.primary)
            }
        } else Spacer(Modifier.height(4.dp))
    }
    items(state.upcoming, key = { "bill_${it.id}" }) { b ->
        BillRow(b, Modifier.padding(horizontal = GUTTER)) { actions.openBill(b.id) }
    }
}

@Composable
private fun BillRow(b: UpcomingBill, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val money = LocalMoney.current
    val amount = if (b.type == TxType.INCOME) money.formatSigned(b.amount) else money.format(b.amount)
    val meta = listOf(Copy.dueLine(b.daysUntil), b.accountName).joinToString(" · ")
    Row(
        modifier
            .fillMaxWidth()
            .bounceClick(label = "Edit ${b.name}", onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = "${b.name}, $amount, $meta" }
            .heightIn(min = 64.dp)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        if (b.type == TxType.TRANSFER) TransferBadge() else CategoryBadge(b.icon, b.color)
        Column(Modifier.weight(1f).clearAndSetSemantics { }, verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(b.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
            Text(meta, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Text(amount, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.clearAndSetSemantics { })
    }
}

private fun LazyListScope.accounts(state: HomeState, actions: HomeActions) {
    section("accounts_header") {
        val fmt = LocalMoney.current
        SectionHeader("Accounts", action = "manage", onAction = actions.accounts)
        if (state.accounts.none { !it.archived }) {
            Spacer(Modifier.height(8.dp))
            Aside("No accounts yet")
        } else {
            Text(
                "Net " + fmt.formatWhole(state.net),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(4.dp))
        }
    }
    items(state.accounts.filter { !it.archived }.take(5), key = { "acc_${it.id}" }) { a ->
        val fmt = LocalMoney.current
        Row(
            Modifier
                .padding(horizontal = GUTTER)
                .fillMaxWidth()
                .bounceClick(label = "${a.name} entries") { actions.openAccount(a.id) }
                .heightIn(min = 60.dp)
                .padding(vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            AccountBadge(a.type)
            Text(a.name, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.weight(1f))
            Text(fmt.format(a.balance), style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onBackground)
        }
    }
}
