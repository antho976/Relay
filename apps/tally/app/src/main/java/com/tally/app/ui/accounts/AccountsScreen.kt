package com.tally.app.ui.accounts

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ReceiptLong
import androidx.compose.material.icons.automirrored.rounded.TrendingUp
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.AccountBadge
import com.tally.app.ui.common.ChromeButton
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.HeroAction
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LegendDot
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.Panel
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.SecondaryAction
import com.tally.app.ui.common.StackedBar
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.StatTile
import com.tally.app.ui.common.ThinBar
import com.tally.app.ui.common.TopBar
import com.tally.app.ui.common.bounceClick
import com.tally.app.ui.nav.AppNav
import com.tally.core.Copy
import com.tally.core.TxType

@Composable
fun AccountsRoute(nav: AppNav) {
    val viewModel: AccountsViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    val actions = remember(nav) {
        AccountsActions(
            back = nav::back,
            add = { nav.accountEdit(0) },
            open = { nav.transactions(accountId = it) },
            transfer = { nav.entry(type = TxType.TRANSFER) },
        )
    }
    AccountsScreen(state, actions)
}

/** Everything the Accounts list can do, as plain lambdas, so it renders in a test with no graph. */
data class AccountsActions(
    val back: () -> Unit = {},
    val add: () -> Unit = {},
    /** Opens an account's entries. */
    val open: (Long) -> Unit = {},
    val transfer: () -> Unit = {},
)

/**
 * Accounts: the net under the light with what is held against what is owed, the largest and the
 * busiest account, then every active account with its share of its side, and the archived ones.
 */
@Composable
fun AccountsScreen(state: AccountsState, actions: AccountsActions) {
    val money = LocalMoney.current
    LazyColumn(
        Modifier.fillMaxSize().navigationBarsPadding(),
        contentPadding = PaddingValues(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        item(key = "head") {
            Column {
                TopBar(onBack = actions.back) { ChromeButton(Icons.Rounded.Add, "Add account", actions.add) }
                PageTitle(
                    "Accounts",
                    Modifier.padding(horizontal = GUTTER).padding(top = 4.dp, bottom = 6.dp),
                    context = "Net " + money.formatWhole(state.net),
                )
            }
        }
        if (state.loaded) {
            item(key = "hero") { NetHero(state, Modifier.padding(horizontal = GUTTER)) }
            item(key = "tiles") { AccountTiles(state, Modifier.padding(horizontal = GUTTER)) }
            item(key = "active") { ActivePanel(state, actions, Modifier.padding(horizontal = GUTTER)) }
            if (state.archived.isNotEmpty()) {
                item(key = "archived") { ArchivedPanel(state, actions, Modifier.padding(horizontal = GUTTER)) }
            }
            if (state.canTransfer) {
                item(key = "transfer") {
                    SecondaryAction("Transfer between accounts", actions.transfer, Modifier.padding(horizontal = GUTTER).fillMaxWidth())
                }
            }
            if (state.active.isEmpty()) {
                item(key = "add") {
                    HeroAction("Add account", actions.add, Modifier.padding(horizontal = GUTTER).fillMaxWidth())
                }
            }
        }
    }
}

/**
 * The net across active accounts as THE figure, the held-against-owed bar with its two amounts,
 * and chips for the entries, the movement since the opening balances and the default account.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun NetHero(state: AccountsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val heldColor = MaterialTheme.colorScheme.primary
    val owedColor = MaterialTheme.colorScheme.onSurfaceVariant
    val segments = remember(state.held, state.owed, heldColor, owedColor) {
        buildList<Pair<Float, Color>> {
            if (state.held > 0) add(state.held.toFloat() to heldColor)
            if (state.owed > 0) add(state.owed.toFloat() to owedColor)
        }
    }
    // At zero the bar still draws, as an empty track a rung above the panel.
    val bar = segments.ifEmpty { listOf(1f to MaterialTheme.colorScheme.surfaceContainerHighest) }
    val count = state.active.size
    HeroPanel(modifier) {
        HeroHead(
            "Net balance",
            end = Copy.plural(count, "account").uppercase(),
        )
        Spacer(Modifier.height(14.dp))
        HeroNumber(money.format(state.net), description = "Net balance, " + money.format(state.net))
        Text(
            if (count == 0) "Nothing to add up yet" else "across " + Copy.plural(count, "active account"),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        StackedBar(
            bar,
            "You hold ${money.formatWhole(state.held)} and owe ${money.formatWhole(state.owed)}",
            height = 12.dp,
        )
        Spacer(Modifier.height(10.dp))
        FlowRow(
            Modifier.fillMaxWidth().clearAndSetSemantics { },
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            LegendDot(heldColor, "Held " + money.formatWhole(state.held))
            LegendDot(owedColor, "Owed " + money.formatWhole(state.owed))
        }
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.AutoMirrored.Rounded.ReceiptLong, Copy.plural(state.entries, "entry", "entries"))
            if (count > 0) StatChip(Icons.AutoMirrored.Rounded.TrendingUp, signedWhole(state.sinceOpening) + " since opening")
            val defaultName = state.defaultName
            if (defaultName != null) StatChip(Icons.Rounded.Star, "New entries start in $defaultName")
        }
    }
}

/** "+$1,234", "−$80", "$0": a movement, signed. */
@Composable
private fun signedWhole(amount: Long): String {
    val money = LocalMoney.current
    return if (amount > 0) "+" + money.formatWhole(amount) else money.formatWhole(amount)
}

/** Two different readings: where the most money sits, and where the most entries land. */
@Composable
private fun AccountTiles(state: AccountsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val largest = state.largest
    val busiest = state.busiest
    TilePair(
        modifier,
        first = { m ->
            StatTile(
                "Largest",
                money.formatWhole(largest?.account?.balance ?: 0L),
                m,
                detail = if (largest == null) "Nothing held yet" else largest.account.name + " · " + shareLine(largest),
            )
        },
        second = { m ->
            StatTile(
                "Most used",
                (busiest?.account?.entryCount ?: 0).toString(),
                m,
                detail = if (busiest == null) "No entries yet" else "entries in " + busiest.account.name,
            )
        },
    )
}

@Composable
private fun ActivePanel(state: AccountsState, actions: AccountsActions, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    Panel(modifier) {
        PanelHeader(
            "Active",
            meta = if (state.active.isEmpty()) null else "NET " + money.formatWhole(state.net),
        )
        if (state.active.isEmpty()) {
            Spacer(Modifier.height(6.dp))
            EmptyNote("Add where your money sits: a bank account, a card, cash")
        } else {
            Spacer(Modifier.height(4.dp))
            state.active.forEach { line -> AccountRow(line, muted = false) { actions.open(line.account.id) } }
        }
    }
}

@Composable
private fun ArchivedPanel(state: AccountsState, actions: AccountsActions, modifier: Modifier = Modifier) {
    Panel(modifier) {
        PanelHeader(
            "Archived",
            meta = Copy.plural(state.archived.size, "account").uppercase(),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(4.dp))
        state.archived.forEach { line -> AccountRow(line, muted = true) { actions.open(line.account.id) } }
    }
}

/**
 * One account, bare in its panel: badge, name, type and entry count, the balance, and (for an
 * active account) a thin bar of its share of what you hold or owe. The whole row opens its entries.
 * Past 1.5x font the balance moves under the name, so neither breaks mid-word.
 */
@Composable
private fun AccountRow(line: AccountLine, muted: Boolean, onClick: () -> Unit) {
    val money = LocalMoney.current
    val a = line.account
    val balance = money.format(a.balance)
    val subtitle = buildList {
        // "Chequing · Chequing" says nothing twice: the type shows only when the name is not it.
        if (!a.name.trim().equals(typeLabel(a.type), ignoreCase = true)) add(typeLabel(a.type))
        add(Copy.plural(a.entryCount, "entry", "entries"))
        if (line.isDefault && !muted) add("default")
    }.joinToString(" · ")
    val share = if (muted) null else shareLine(line)
    val strong = if (muted) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onBackground
    val stacked = LocalDensity.current.fontScale > 1.5f
    Row(
        Modifier
            .fillMaxWidth()
            .bounceClick(label = "${a.name} entries", onClick = onClick)
            .semantics(mergeDescendants = true) {
                contentDescription = listOfNotNull(a.name, balance, subtitle, share).joinToString(", ")
            }
            .heightIn(min = 64.dp)
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        AccountBadge(a.type, Modifier.alpha(if (muted) 0.6f else 1f))
        Column(Modifier.weight(1f).clearAndSetSemantics { }, verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(a.name, style = MaterialTheme.typography.bodyLarge, color = strong)
            if (stacked) Text(balance, style = MaterialTheme.typography.titleMedium, color = strong)
            Text(subtitle, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            if (share != null) {
                Spacer(Modifier.height(6.dp))
                ThinBar(
                    line.share,
                    if (a.balance < 0) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.primary,
                    share,
                )
            }
        }
        if (!stacked) {
            Text(balance, style = MaterialTheme.typography.titleMedium, color = strong, modifier = Modifier.clearAndSetSemantics { })
        }
    }
}
