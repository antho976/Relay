package com.margin.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AccountBalance
import androidx.compose.material.icons.rounded.CalendarMonth
import androidx.compose.material.icons.rounded.Category
import androidx.compose.material.icons.rounded.CloudOff
import androidx.compose.material.icons.rounded.Contrast
import androidx.compose.material.icons.rounded.CurrencyExchange
import androidx.compose.material.icons.rounded.Lock
import androidx.compose.material.icons.rounded.NoAccounts
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.Science
import androidx.compose.material.icons.rounded.SettingsBackupRestore
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.margin.app.ui.common.Dates
import com.margin.app.ui.common.GUTTER
import com.margin.app.ui.common.Group
import com.margin.app.ui.common.GroupRow
import com.margin.app.ui.common.HeroPanel
import com.margin.app.ui.common.LocalMoney
import com.margin.app.ui.common.PANEL_GAP
import com.margin.app.ui.common.StatChip
import com.margin.app.ui.common.StatTile
import com.margin.app.ui.nav.AppNav
import com.margin.core.Copy

@Composable
fun SettingsRoute(nav: AppNav) {
    val viewModel: SettingsViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    val actions = remember(nav) {
        SettingsActions(
            back = nav::back,
            appearance = nav::appearance,
            format = nav::format,
            accounts = nav::accounts,
            categories = nav::categories,
            data = nav::data,
        )
    }
    SettingsScreen(state, actions)
}

/** Everything Settings can open, as plain lambdas. */
data class SettingsActions(
    val back: () -> Unit = {},
    val appearance: () -> Unit = {},
    val format: () -> Unit = {},
    val accounts: () -> Unit = {},
    val categories: () -> Unit = {},
    val data: () -> Unit = {},
)

/**
 * Settings: what this phone holds under the one lit panel, the net and the period as tiles, then
 * the grouped rows into each settings page, each carrying its current value.
 */
@Composable
fun SettingsScreen(state: SettingsState, actions: SettingsActions) {
    LazyColumn(
        Modifier.fillMaxSize().navigationBarsPadding(),
        contentPadding = PaddingValues(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        item(key = "head") {
            PageHead("Settings", onBack = actions.back)
        }
        item(key = "hero") { LedgerHero(state, Modifier.padding(horizontal = GUTTER)) }
        item(key = "tiles") { SettingsTiles(state, Modifier.padding(horizontal = GUTTER)) }
        item(key = "general") { GeneralGroup(state, actions, Modifier.padding(horizontal = GUTTER).padding(top = 10.dp)) }
        item(key = "money") { MoneyGroup(state, actions, Modifier.padding(horizontal = GUTTER).padding(top = 10.dp)) }
        item(key = "data") { DataGroupRows(state, actions, Modifier.padding(horizontal = GUTTER).padding(top = 10.dp)) }
    }
}

/** What lives on this phone: the entry count as the figure, and the privacy facts as chips. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun LedgerHero(state: SettingsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val count = remember(state.entries, money.locale) { countText(state.entries, money.locale) }
    HeroPanel(modifier) {
        HeroHead(Icons.Rounded.PhoneAndroid, "On this phone", end = "OFFLINE")
        Spacer(Modifier.height(12.dp))
        HeroNumber(count, description = Copy.plural(state.entries, "entry", "entries"))
        Text(
            if (state.entries == 0) {
                "No entries yet. What you log is kept here and nowhere else"
            } else {
                (if (state.entries == 1) "entry" else "entries") + " across " + Copy.plural(state.accounts, "account") +
                    ", kept here and nowhere else"
            },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.Rounded.CloudOff, "Works offline")
            StatChip(Icons.Rounded.NoAccounts, "No sign-in")
            StatChip(Icons.Rounded.Lock, "Leaves only when you export")
            if (state.settings.sampleLoaded) StatChip(Icons.Rounded.Science, "Sample data")
        }
    }
}

/** Two different readings: where the money sits overall, and which budget period runs now. */
@Composable
private fun SettingsTiles(state: SettingsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    TilePair(
        modifier,
        first = { m ->
            StatTile(
                Icons.Rounded.AccountBalance,
                "Net",
                money.formatWhole(state.net),
                m,
                detail = if (state.accounts == 0) "No accounts yet" else "Across " + Copy.plural(state.accounts, "account"),
            )
        },
        second = { m ->
            StatTile(
                Icons.Rounded.CalendarMonth,
                "Period",
                Dates.period(state.period, state.today),
                m,
                detail = "Resets " + Dates.short(state.period.endExclusive, state.today),
            )
        },
    )
}

@Composable
private fun GeneralGroup(state: SettingsState, actions: SettingsActions, modifier: Modifier = Modifier) {
    val s = state.settings
    val appearance: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Appearance",
            shape,
            subtitle = appearanceSummary(s.amoled, s.accentEnabled, s.accent),
            leading = { RowBadge(Icons.Rounded.Contrast) },
            onClick = actions.appearance,
        )
    }
    val format: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Currency & dates",
            shape,
            subtitle = formatSummary(s.currency, s.monthStartDay, s.weekStartsMonday),
            leading = { RowBadge(Icons.Rounded.CurrencyExchange) },
            onClick = actions.format,
        )
    }
    Group(rows = listOf(appearance, format), modifier = modifier, title = "General")
}

@Composable
private fun MoneyGroup(state: SettingsState, actions: SettingsActions, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val accounts: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Accounts",
            shape,
            subtitle = accountsSummary(state.accounts, money.formatWhole(state.net)),
            leading = { RowBadge(Icons.Rounded.AccountBalance) },
            onClick = actions.accounts,
        )
    }
    val categories: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Categories",
            shape,
            subtitle = categoriesSummary(state.spendingCategories, state.incomeCategories),
            leading = { RowBadge(Icons.Rounded.Category) },
            onClick = actions.categories,
        )
    }
    val plan = listOfNotNull(
        Copy.plural(state.budgets, "budget").takeIf { state.budgets > 0 },
        Copy.plural(state.bills, "bill").takeIf { state.bills > 0 },
        Copy.plural(state.goals, "goal").takeIf { state.goals > 0 },
    ).joinToString(" · ")
    Group(
        rows = listOf(accounts, categories),
        modifier = modifier,
        title = "Money",
        footer = if (plan.isEmpty()) "No budgets, bills or goals yet" else "$plan in your plan",
    )
}

@Composable
private fun DataGroupRows(state: SettingsState, actions: SettingsActions, modifier: Modifier = Modifier) {
    val backup: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Backup, export and import",
            shape,
            subtitle = "JSON backup · CSV · sample data",
            leading = { RowBadge(Icons.Rounded.SettingsBackupRestore) },
            onClick = actions.data,
        )
    }
    Group(
        rows = listOf(backup),
        modifier = modifier,
        title = "Data",
        footer = "Margin ${state.version} · offline, no account, no internet permission",
    )
}
